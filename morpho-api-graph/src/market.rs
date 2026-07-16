
use crate::{HttpClient}; 
use crate::queries::markets_query; 
use crate::types::{MarketItem, MarketsResult}; 
use serde_json::to_string_pretty;
use std::str::FromStr;
use alloy_primitives::{Address, U256, FixedBytes};
use std::fs;

const MARKETS_CACHE: &str = "data/markets.json";

pub async fn fetch_all_market(chain_id: u32) -> anyhow::Result<Vec<MarketItem>> {
    let client = HttpClient::new();
    
    let result = client.query(&markets_query(chain_id, 500.0)).await?;
    let markets: MarketsResult = serde_json::from_value(result)?;
    
    // sauvegarde
    fs::create_dir_all("data")?;
    
    Ok(markets.markets.items)
}

pub async fn fetch_or_load_markets(chain_id: u32) -> anyhow::Result<Vec<MarketItem>> {
    match fetch_all_market(chain_id).await {
        Ok(markets) => Ok(markets),
        Err(e) => {
            eprintln!("API error: {e}, loading from cache...");
            let json = fs::read_to_string(MARKETS_CACHE)?;
            Ok(serde_json::from_str(&json)?)
        }
    }
}

pub async fn load_markets(chain_id: u32) -> anyhow::Result<Vec<MarketItem>> {
    let json = fs::read_to_string(MARKETS_CACHE)?;
    Ok(serde_json::from_str(&json)?)
}




