use alloy_primitives::{Address,FixedBytes, U256};
use crate::morpho::{hf, types::MarketParam}; 

#[derive(Debug, Clone, PartialEq)]
pub struct BorrowPosition {
    pub market_id: FixedBytes<32>,
    pub address: Address,
    pub borrow_shares: U256,
    pub borrow_assets_usd: f64,
    pub collateral_assets: U256,
    pub cached_hf: Option<U256>,
    pub onchain_checked: bool, 
}


impl  BorrowPosition {

   pub fn health_factor(
    &self,
    total_borrow_asset: U256,
    total_borrow_shares: U256,
    lltv: U256,
    oracle_price: U256,
) -> Option<U256> {
    hf(
        self.collateral_assets,
        self.borrow_shares,
        total_borrow_shares,
        total_borrow_asset,
        lltv,
        oracle_price,
    )
}
}

