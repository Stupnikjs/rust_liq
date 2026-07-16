#![allow(dead_code, unused_variables, unused_imports)]
// api/types.rs
use super::number::Number;
use serde::{Deserialize, Serialize};


#[derive(Debug, Deserialize, Serialize)]
pub struct Asset {
    pub address: String,
    pub symbol: String,
    pub decimals: i32,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PageInfo {
    pub count: i32,
    #[serde(rename = "countTotal")]
    pub count_total: i32,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PositionItem {
    pub user: PositionUser,
    pub state: PositionState,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PositionUser {
    pub address: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PositionState {
    #[serde(rename = "borrowShares")]
    pub borrow_shares: Number,
    #[serde(rename = "borrowAssetsUsd")]
    pub borrow_assets_usd: Number,
    pub collateral: Number,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PositionsResult {
    #[serde(rename = "marketPositions")]
    pub market_positions: MarketPositions,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct MarketPositions {
    pub items: Vec<PositionItem>,
    #[serde(rename = "pageInfo")]
    pub page_info: PageInfo,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct MarketItem {
    #[serde(rename = "marketId")]
    pub id: String,
    #[serde(rename = "creationTimestamp")]
    pub creation_timestamp: i64,
    #[serde(rename = "oracleAddress")]
    pub oracle_address: String,
    pub lltv: Number,
    #[serde(rename = "irmAddress")]
    pub irm: String,
    #[serde(rename = "loanAsset")]
    pub loan_asset: Asset,
    #[serde(rename = "collateralAsset")]
    pub collateral_asset: Option<Asset>, // null possible → Option, pas de pointeur
    pub state: MarketState,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct MarketState {
    #[serde(rename = "supplyAssetsUsd")]
    pub supply_assets_usd: Number,
    #[serde(rename = "borrowAssetsUsd")]
    pub borrow_assets_usd: Number,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct MarketsResult {
    pub markets: MarketsInner,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct MarketsInner {
    pub items: Vec<MarketItem>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct LiquidationItem {
    pub hash: String,
    pub timestamp: Number,
    #[serde(rename = "type")]
    pub kind: String, // `type` est un mot réservé Rust, donc rename obligatoire
    pub data: LiquidationData,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct LiquidationData {
    #[serde(rename = "seizedAssets")]
    pub seized_assets: Number,
    #[serde(rename = "repaidAssets")]
    pub repaid_assets: Number,
    #[serde(rename = "seizedAssetsUsd")]
    pub seized_assets_usd: Number,
    #[serde(rename = "repaidAssetsUsd")]
    pub repaid_assets_usd: Number,
    #[serde(rename = "badDebtAssetsUsd")]
    pub bad_debt_assets_usd: Number,
    pub liquidator: String,
    pub market: LiquidationMarket,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct LiquidationMarket {
    #[serde(rename = "marketId")]
    pub market_id: String,
    #[serde(rename = "loanAsset")]
    pub loan_asset: Asset,
    #[serde(rename = "collateralAsset")]
    pub collateral_asset: Asset,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct LiquidationsResult {
    pub transactions: LiquidationTransactions,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct LiquidationTransactions {
    pub items: Vec<LiquidationItem>,
    #[serde(rename = "pageInfo")]
    pub page_info: PageInfo,
}