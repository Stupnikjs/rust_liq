#![allow(dead_code, unused_variables, unused_imports)]
use alloy_primitives::{FixedBytes, U256, Address}; 
use anyhow::Context;
use serde_json::to_string_pretty;
use std::fs;
use hex;
use tokio::runtime::Id; 
use std::str::FromStr;
use crate::types::{MarketItem, PositionItem, PositionsResult};
use crate::{HttpClient, pos};
use crate::queries::positions_query;


pub async fn fetch_all_positions(
    market_id: FixedBytes<32>,
    chain_id: u32,
) -> anyhow::Result<Vec<PositionItem>> {
    let client = HttpClient::new();
    let mut all = Vec::new();
    let mut skip: i64 = 0;
    let id_string = format!("{:?}", market_id);

    loop {
        let result: PositionsResult = client
            .query(&positions_query(&id_string, chain_id, skip))
            .await
            .with_context(|| format!("fetch positions page skip={skip}"))?;

        let mp = result.market_positions;
        all.extend(mp.items);

        skip += mp.page_info.count as i64;
        if skip >= mp.page_info.count_total as i64 {
            break;
        }
    }
    Ok(all)
}





