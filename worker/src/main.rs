#![allow(dead_code, unused_variables, unused_imports)]

use tokio::time::{sleep, Duration};



#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
   worker::run().await 
}


/*   note 

thread 'main' (11436) panicked at connector/src/lib.rs:140:109:
called `Result::unwrap()` on an `Err` value: no rpc available
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
backtest_store: writer arrêté => backtest ligne 91

*/


