pub mod encode;
pub mod build;

use alloy_primitives::{U256, Address};
use encode::encode_liquidate; 
use build::build_steps; 
use crate::swap::PoolEdge;
use crate::{cache::positions::BorrowPosition};
use connector::Connector;
use morpho::types::MarketParam; 

pub async fn liquidate(
    conn: &Connector,
    pos: BorrowPosition,
    route: PoolEdge,
    mparam: MarketParam,
    liquidator_addr: Address,
) {
    let swap_steps: Vec<PoolEdge> = vec![route.clone()];

    let steps = match build_steps(&swap_steps, liquidator_addr) {
        Ok(steps) => steps,
        Err(e) => {
            eprintln!("error build step {}", e);
            return;
        }
    };
   let wc_amount_in = route.wc_amount_in; 
    let mut seized_assets = pos.collateral_assets; // ajust with slippage
    if seized_assets > wc_amount_in {
        seized_assets = wc_amount_in;
    }

    let calldata = encode_liquidate(&mparam, pos.address, seized_assets, U256::ZERO, steps, U256::ZERO);
    match conn.call_raw(liquidator_addr, calldata.clone()).await {
    Ok(_) => {
        // simulation OK, on peut envoyer
    }
    Err(e) => {
        eprintln!("simulation failed for {:?}: {}", pos.market_id, e);
        return;
    }
}

    let tx_hash = conn.send_tx(liquidator_addr, calldata).await;

    // save tx_hash + ts for backtest 
}