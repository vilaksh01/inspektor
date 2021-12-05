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

mod error;
pub mod ctx;
pub mod query_rewriter;
pub mod rule_engine;

// TODO: things that needs to be revisied
// 1) table function
// 2) not supported sql by sql parser
// SELECT
        //     id,
        //     CASE
        //         WHEN rating~E'^\\d+$' THEN
        //             CAST (rating AS INTEGER)
        //         ELSE
        //             0
        //         END as rating
        // FROM
        //     ratings