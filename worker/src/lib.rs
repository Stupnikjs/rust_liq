#![allow(dead_code, unused_variables, unused_imports)]

use std::sync::Arc;



pub mod cache; 
pub mod runner; 
pub mod config;
pub mod swap;
pub mod liquidate;
pub mod backtest;




pub async fn run() -> Result<(), Box<dyn std::error::Error>> {

    let chain = std::env::args().nth(1).unwrap_or_else(|| {
    eprintln!("missing <chain>"); 
    std::process::exit(1);
    });

    let chainint:u64 = chain.parse()?;
    let mut runner  = runner::Runner::new(chainint).await.expect("failed runner new func");
    runner.init().await.expect(""); 
    let runner = Arc::new(runner);
   tokio::select! {
        res = runner.run() => {
            if let Err(e) = res {
                eprintln!("FATAL: {e}");
                std::process::exit(1);
            }
        }
        _ = tokio::signal::ctrl_c() => {
            eprintln!("received ctrl-c, shutting down");
        }
    }
    Ok(())
}
