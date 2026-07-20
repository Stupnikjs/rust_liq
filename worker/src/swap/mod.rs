use alloy::primitives::{Address, U256};
use std::time::Instant;
use morpho::types::{MarketParam}; 
use crate::swap::abi::uni::encode_exact_input_single_uni; 
use crate::swap::abi::pankake::encode_exact_input_single_pancake; 
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

pub mod quoter;
pub mod routes; 
pub mod abi; 




#[derive(Debug, Clone)]
pub struct SwapStep {
    pub target: Address,
    pub data: Vec<u8>,
    pub token_in: Address,
    pub token_out: Address,
    pub amount_in_offset: U256,
}



// --- Address <-> String ---

fn ser_addr<S: Serializer>(a: &Address, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&a.to_string())
}

fn de_addr<'de, D: Deserializer<'de>>(d: D) -> Result<Address, D::Error> {
    let s = String::deserialize(d)?;
    Address::from_str(&s).map_err(serde::de::Error::custom)
}

// --- U256 <-> String ---

fn ser_u256<S: Serializer>(v: &U256, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&v.to_string())
}

fn de_u256<'de, D: Deserializer<'de>>(d: D) -> Result<U256, D::Error> {
    let s = String::deserialize(d)?;
    U256::from_str(&s).map_err(serde::de::Error::custom)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolEdge {
    #[serde(serialize_with = "ser_addr", deserialize_with = "de_addr")]
    pub token_in: Address,
    #[serde(serialize_with = "ser_addr", deserialize_with = "de_addr")]
    pub token_out: Address,
    #[serde(serialize_with = "ser_addr", deserialize_with = "de_addr")]
    pub router: Address,
    pub fee: u32,
    pub wc_slippage: f64,
    #[serde(serialize_with = "ser_u256", deserialize_with = "de_u256")]
    pub wc_amount_in: U256,
    #[serde(serialize_with = "ser_u256", deserialize_with = "de_u256")]
    pub wc_amount_out: U256,
    pub calibrated_at: u64, // unix ms
    pub dex_name: String,
    pub amount_in_offset: i64,
    #[serde(serialize_with = "ser_u256", deserialize_with = "de_u256")]
    pub price_at_quote: U256,
}

// --- helpers timestamp (remplacent Instant::now() / .elapsed()) ---

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

pub fn age_ms(calibrated_at: u64) -> u64 {
    now_ms().saturating_sub(calibrated_at)
}

// --- (de)serialisation Vec<PoolEdge> ---

pub fn edges_to_json(edges: &[PoolEdge]) -> serde_json::Result<String> {
    serde_json::to_string_pretty(edges)
}

pub fn edges_from_json(json: &str) -> serde_json::Result<Vec<PoolEdge>> {
    serde_json::from_str(json)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let edge = PoolEdge {
            token_in: Address::ZERO,
            token_out: Address::ZERO,
            router: Address::ZERO,
            fee: 3000,
            wc_slippage: 0.005,
            wc_amount_in: U256::from(1_000_000u64),
            wc_amount_out: U256::from(999_500u64),
            calibrated_at: now_ms(),
            dex_name: "aerodrome".to_string(),
            amount_in_offset: 4,
            price_at_quote: U256::from(1u64) << 96,
        };

        let json = edges_to_json(&[edge.clone()]).unwrap();
        let back = edges_from_json(&json).unwrap();
        assert_eq!(back[0].wc_amount_in, edge.wc_amount_in);
        assert_eq!(back[0].token_in, edge.token_in);
    }
}