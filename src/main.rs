#![feature(async_closure)]
// Copyright 2021 Balaji (rbalajis25@gmail.com)
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

mod apiproto;
mod auditlog;
mod config;
mod policy_evaluator;
mod postgres_driver;
mod sql;
use apiproto::api::*;
use apiproto::api_grpc::*;
use clap::{App, Arg};
use env_logger;
use futures;
use futures::prelude::*;
use grpcio::{ChannelBuilder, EnvBuilder};
use log::*;
use std::sync::Arc;
use tokio::sync::watch;

fn main() {
    env_logger::init();
    let app = App::new("inspektor")
        .version("0.0.1")
        .author("Balaji <rbalajis25@gmail.com>")
        .about("inspector is used to autheticate your data layer")
        .arg(
            Arg::with_name("config_file")
                .short("c")
                .long("config_file")
                .required(true)
                .takes_value(true),
        )
        .get_matches();
    let config_path = app.value_of("config_file").unwrap();
    let mut config = config::read_config(&std::path::PathBuf::from(config_path)).unwrap();
    config.validate().unwrap();

    let env = Arc::new(EnvBuilder::new().build());
    let ch = ChannelBuilder::new(env).connect(config.controlplane_addr.as_ref().unwrap());
    let client = InspektorClient::new(ch);

    let token = config.secret_token.as_ref().unwrap().clone();
    let get_call_opt = Box::new(move || {
        // create meta which is used for header based authentication.
        let mut meta_builder = grpcio::MetadataBuilder::new();
        meta_builder.add_str("auth-token", &token.clone()).unwrap();
        let meta = meta_builder.build();
        return grpcio::CallOption::default().headers(meta);
    });

    // retrive data source. so that we can use that to give as input to evaluate policies.
    // if we don't get any data then there is something wrong with control plane or provided
    // secret token.
    let source = client
        .get_data_source_opt(&Empty::default(), get_call_opt())
        .expect("check whether given secret token is valid or check control plane");
    // retrive all the integration config.
    let mut integration_config = client
        .get_integration_config_opt(&Empty::new(), get_call_opt())
        .expect("error while retriving integration config");
    let audit_sender = auditlog::start_audit_worker(integration_config.take_cloud_watch_config());
    // prepare for wathcing polices.
    let mut policy_reciver = client
        .policy_opt(&Empty::default(), get_call_opt())
        .unwrap();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (policy_broadcaster, policy_watcher) = watch::channel(Vec::<u8>::new());
    // wait for policy in a different thread. we can use the same thread for other common telementry
    // data.
    let policy_client = client.clone();
    std::thread::spawn(move || {
        info!("strated watching for polices");
        rt.block_on(async {
            'policy_watcher: loop {
                let result = match policy_reciver.try_next().await {
                    Ok(result) => result,
                    Err(e) => {
                        error!("error while retriving policies {:?}", e);
                        'policy_retrier: loop {
                            policy_reciver =
                                match policy_client.policy_opt(&Empty::default(), get_call_opt()) {
                                    Ok(receiver) => receiver,
                                    Err(e) => {
                                        error!("error while retriving policy receiver {:?}", e);
                                        continue 'policy_retrier;
                                    }
                                };
                            continue 'policy_watcher;
                        }
                    }
                };
                let policy = match result {
                    Some(policy) => policy,
                    None => continue,
                };
                if let Err(e) = policy_broadcaster.send(policy.wasm_byte_code) {
                    error!(
                        "error while sending policy to policy watchers. err: {:?}",
                        e
                    );
                }
            }
        });
    });

    let driver = postgres_driver::driver::PostgresDriver {
        postgres_config: config.postgres_config.unwrap(),
        policy_watcher: policy_watcher,
        datasource: source,
        client: client,
        token: config.secret_token.as_ref().unwrap().clone(),
        audit_sender: audit_sender,
    };
    driver.start();
}
