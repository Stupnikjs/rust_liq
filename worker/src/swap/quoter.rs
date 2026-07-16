// src/swap/uniswap.rs
use std::time::{Duration, Instant};
use alloy::primitives::{Address, U256, Bytes};
use crate::swap::{PoolEdge, now_ms};
use connector::Connector;
use eth_core::encode::{encode_address, encode_uint256,selector}; 
use crate::swap::abi::uni::encode_quote_single_exact_input; 

const UNI_FEES: [u32; 4] = [100, 500, 3000, 10000];

pub struct UniswapV3 {
    pub quoter: Address,
    pub router: Address,
    pub rate_limit: u64,
    pub name: String,
}

impl UniswapV3 {
    pub fn new(quoter: Address, router: Address, rate_limit: u64, name: String) -> Self {
        Self { quoter, router, rate_limit, name }
    }

    // binary search sur amountIn comme le Go
    pub async fn best_amount_in(
        &self,
        connector: &Connector,
        token_in: Address,
        token_out: Address,
        amount_in: U256,
        oracle_price: U256,
        max_slippage: f64,
    ) -> Option<PoolEdge> {
        let mut lo = U256::from(1u64);
        let mut hi = amount_in;
        let mut best: Option<PoolEdge> = None;

        for _ in 0..12 {
            if lo > hi { break; }
            let mid = (lo + hi) >> 1;

            match self.uni_quote(connector, token_in, token_out, mid, oracle_price, max_slippage).await {
                Some(edge) => {
                    best = Some(edge);
                    lo = mid + U256::from(1u64);
                }
                None => {
                    if mid == U256::ZERO { break; }
                    hi = mid - U256::from(1u64);
                }
            }
        }

        best
    }

    // essaie tous les fee tiers, retourne le meilleur amountOut
    async fn uni_quote(
        &self,
        connector: &Connector,
        token_in: Address,
        token_out: Address,
        amount_in: U256,
        oracle_price: U256,
        max_slippage: f64,
    ) -> Option<PoolEdge> {
        let mut best: Option<PoolEdge> = None;

        for &fee in &UNI_FEES {
            tokio::time::sleep(Duration::from_millis(self.rate_limit)).await;

            let Some(edge) = self.quote_call(
                connector, token_in, token_out, amount_in, oracle_price, fee,
            ).await else { continue }; 
            

            if edge.wc_slippage > max_slippage {
                continue;
            }

            match &best {
                None => best = Some(edge),
                Some(b) if edge.wc_amount_out > b.wc_amount_out => best = Some(edge),
                _ => {}
            }
        }

        best
    }

    // appel RPC — à implémenter avec ABI manuel
    async fn quote_call(
        &self,
        connector: &Connector,
        token_in: Address,
        token_out: Address,
        amount_in: U256,
        oracle_price: U256,
        fee: u32,
    ) -> Option<PoolEdge> {
        // TODO: encoder quoteExactInputSingle + eth_call + décoder amountOut
        let amount_out = self.eth_call_quote(connector,  token_in, token_out, amount_in, fee).await?;
     

        Some(PoolEdge {
            token_in,
            token_out,
            router: self.router,
            fee,
            wc_slippage: compute_slippage(amount_in, amount_out, oracle_price),
            wc_amount_in: amount_in,
            wc_amount_out: amount_out,
            calibrated_at: now_ms(),
            dex_name: self.name.clone(),
            amount_in_offset: 164, // uniswap offset
            price_at_quote: oracle_price,
        })
    }

   async fn eth_call_quote(
    &self,
    connector: &Connector,
    token_in: Address,
    token_out: Address,
    amount_in: U256,
    fee: u32,
) -> Option<U256> {
    let calldata = encode_quote_single_exact_input(token_in, token_out, fee, amount_in);
    let resp = connector.call_raw(self.quoter, calldata).await;
    match resp {
        Ok(bytes) => {
            if bytes.len() < 32 {
                return None;
            }
            // amountOut = premier slot du tuple retourné
            let amount_out = U256::from_be_slice(&bytes[0..32]);
            Some(amount_out)
        }
        Err(e) => {
            None
        }
    }
}
}

pub fn compute_slippage(amount_in: U256, amount_out: U256, oracle_price: U256) -> f64 {
    if oracle_price.is_zero() { return 0.0; }

    // expected_out = amount_in * oracle_price / 1e36
    let scale = U256::from(10u64).pow(U256::from(36u64));
    let expected_out = amount_in * oracle_price / scale;

    if expected_out.is_zero() { return 0.0; }

    if amount_out >= expected_out { return 0.0; }

    let diff = expected_out - amount_out;

    // conversion en f64
    let diff_f: f64 = diff.to_string().parse().unwrap_or(0.0);
    let exp_f: f64 = expected_out.to_string().parse().unwrap_or(1.0);

    (diff_f / exp_f) * 100.0
}




