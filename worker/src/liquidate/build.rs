// src/liquidate/build.rs
use alloy::primitives::{Address, Bytes, U256};

use crate::swap::{SwapStep,PoolEdge};
use crate::swap::abi;
use crate::morpho::types::MarketParam;
use crate::liquidate::encode::encode_liquidate; 

pub struct Liquidable {
    pub borrower: Address,
    pub seize_assets: U256,
    pub repay_shares: U256,
}

pub fn build_steps(route: &[PoolEdge], liquidator_addr: Address) -> Result<Vec<SwapStep>, anyhow::Error> {
    route.iter().map(|hop| {
        let data = match hop.dex_name.as_str() {
            "UNIV3" => abi::uni::encode_exact_input_single_uni(
                hop.token_in,
                hop.token_out,
                hop.fee,
                liquidator_addr,
            ),
            "PANCAKE" => abi::pankake::encode_exact_input_single_pancake(
                hop.token_in,
                hop.token_out,
                hop.fee,
                liquidator_addr,
            ),
            _ => return Err(anyhow::anyhow!("DEX non supporté: {}", hop.dex_name)),
        };

        Ok(SwapStep {
            target: hop.router,
            data: data.to_vec(),
            token_in: hop.token_in,
            token_out: hop.token_out,
            amount_in_offset: match hop.dex_name.as_str() {
                "UNIV3"   => U256::from(132u64),
                "PANCAKE" => U256::from(164u64),
                _         => U256::ZERO,
            },
        })
    }).collect()
}

pub fn to_liquidation_calldata(
    liquidator_addr: Address,
    liquidable: &Liquidable,
    market: &MarketParam,
    route: &[PoolEdge],
) -> Result<Bytes, anyhow::Error> {


    let steps = build_steps(route, liquidator_addr)?;

    Ok(encode_liquidate(
        market,
        liquidable.borrower,
        liquidable.seize_assets,
        liquidable.repay_shares,
        steps,
        U256::ZERO,
    ))
}

