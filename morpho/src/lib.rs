#![allow(dead_code, unused_variables, unused_imports)]
use alloy::primitives::U256;


pub mod types;
pub mod calls; 
pub mod utils;

fn wad() -> U256 {
    U256::from(10u64).pow(U256::from(18))
}
fn ten_pow(exp: u32) -> U256 {
    U256::from(10u64).pow(U256::from(exp))
}
 
pub fn liquidation_incentive_factor(lltv: U256) -> U256 {
    let wad = wad();
    let max_lif = wad / U256::from(100) * U256::from(15); // 0.15e18
 
    if lltv.is_zero() {
        return max_lif;
    }
 
    let lif_raw = wad * wad / lltv;
    if lif_raw <= wad {
        return U256::ZERO;
    }
    let lif = lif_raw - wad;
 
    if lif > max_lif { max_lif } else { lif }
}

pub fn borrow_assets_from_shares(
    pos_shares: U256,
    tot_shares: U256,
    tot_borrow_assets: U256,
) -> U256 {
    if tot_shares.is_zero() || tot_borrow_assets.is_zero() {
        return U256::ZERO;
    }
    pos_shares * tot_borrow_assets / tot_shares
}
 
 
pub fn compute_seized_asset(
    borrow_shares: U256,
    total_borrow_assets: U256,
    total_borrow_shares: U256,
    lltv: U256,
) -> U256 {
    let wad = wad();
    let repay_assets = borrow_assets_from_shares(borrow_shares, total_borrow_assets, total_borrow_shares);
    let lif = liquidation_incentive_factor(lltv);
    repay_assets * (wad + lif) / wad
}
 
pub fn collateral_assets_in_loan(seized_assets: U256, collateral_price: U256) -> U256 {
    seized_assets * collateral_price / ten_pow(36)
}
 
pub fn hf(
    collateral_assets: U256,
    borrow_shares: U256,
    total_borrow_shares: U256,
    total_borrow_assets: U256,
    lltv: U256,
    oracle_price: U256,
) -> Option<U256> {
    let borrow_assets =
        borrow_assets_from_shares(borrow_shares, total_borrow_shares, total_borrow_assets);
 
    if borrow_assets.is_zero() || collateral_assets.is_zero() {
        return None;
    }
 
    let numerator = collateral_assets * oracle_price * lltv;
    let denominator = borrow_assets * ten_pow(36);
    Some(numerator / denominator)
}
