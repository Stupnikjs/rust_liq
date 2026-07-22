use std::env::Args;

use alloy::primitives::{Address, U256};
use alloy::sol_types::sol_data::{self, FixedBytes};
use alloy_primitives::{Bytes, address};
use alloy::providers::Provider;
use alloy::network::Ethereum;
use eth_core::encode::{selector,encode_calldata}; 
use eth_core::traits::{CallRaw, RpcKind};

// market() call to morpho 


#[derive(Debug)]
pub struct MarketStatsCall {
    pub total_supply_assets: U256,
    pub total_supply_shares: U256,
    pub total_borrow_assets: U256,
    pub total_borrow_shares: U256,
    pub last_update: U256,
    pub fee: U256,
}

type MarketTuple = (
    sol_data::Uint<128>,
    sol_data::Uint<128>,
    sol_data::Uint<128>,
    sol_data::Uint<128>,
    sol_data::Uint<128>,
    sol_data::Uint<128>,
);

pub fn decode_market_stats(data: &[u8]) -> Result<MarketStatsCall, anyhow::Error> {
    if data.len() < 192 {
       
        return Err(anyhow::anyhow!("response too short"));
    }

    // chaque slot = 32 bytes, la valeur uint128 est dans les 16 bytes de droite
    let read_u128 = |slot: usize| -> U256 {
        let offset = slot * 32;
        U256::from_be_slice(&data[offset..offset + 32])
    };

    Ok(MarketStatsCall {
        total_supply_assets:  read_u128(0),
        total_supply_shares:  read_u128(1),
        total_borrow_assets:  read_u128(2),
        total_borrow_shares:  read_u128(3),
        last_update:          read_u128(4),
        fee:                  read_u128(5),
    })
}

pub async fn market_call<C>(
    conn: &C,
    rpc:RpcKind,
    morpho_addr: Address,
    market_id: &[u8],
) -> Result<MarketStatsCall, anyhow::Error>
where
    C: CallRaw,
{
    let selector = selector("market(bytes32)");

    let calldata = encode_calldata(selector, market_id);
    let from = address!("78D3FEc647f35E5D413597D217C5E0D9605acE3E"); 
    let resp = conn
        .call_raw(false, from , morpho_addr, calldata)
        .await
        .map_err(|e| anyhow::anyhow!("market call failed: {:?}", e))?;

    decode_market_stats(&resp)
}


// position()

#[derive(Debug)]
pub struct PositionCall {
    pub supply_shares: U256,
    pub borrow_shares: U256,
    pub collateral: U256,
}

pub fn decode_position(data: &[u8]) -> Result<PositionCall, anyhow::Error> {
    if data.len() < 96 {
        return Err(anyhow::anyhow!("response too short"));
    }

    let read = |slot: usize| -> U256 {
        let offset = slot * 32;
        U256::from_be_slice(&data[offset..offset + 32])
    };

    Ok(PositionCall {
        supply_shares: read(0),
        borrow_shares: read(1),
        collateral: read(2),
    })
}


pub async fn position_call<C>(
    conn: &C,
    rpc: RpcKind,
    morpho_addr: Address,
    market_id: &[u8],
    user: Address,
) -> Result<PositionCall, anyhow::Error>
    where
        C: CallRaw
    {

    let sel = selector("position(bytes32,address)");
    let mut args: Vec<u8> = Vec::new(); 

    // bytes32
    args.extend_from_slice(market_id);
    // address -> padding ABI
    let mut addr = [0u8; 32];
    addr[12..].copy_from_slice(user.as_slice());
    args.extend_from_slice(&addr);

    let  calldata = encode_calldata(sel, &args);
    let from = address!("78D3FEc647f35E5D413597D217C5E0D9605acE3E"); 
    let resp = conn
        .call_raw(false, from, morpho_addr, calldata)
        .await
        .map_err(|e| anyhow::anyhow!("position call failed: {:?}", e))?;

    decode_position(&resp)
}




// price() call on oracle morpho 
// exponent = 36 + loan_decimals - collateral_decimals

pub async fn oracle_call<C>(conn: &C, rpc: RpcKind, oracle_addr: Address)-> Result<U256, anyhow::Error>
    where
        C: CallRaw
    {
    let from = address!("78D3FEc647f35E5D413597D217C5E0D9605acE3E");    
    let selector = selector("price()");
    let calldata = encode_calldata(selector, &[]);
    let resp = conn.call_raw(false, from, oracle_addr, calldata).await
        .map_err(|e| anyhow::anyhow!("oracle_call failed for {}: {:?}", oracle_addr, e))?;
    
    decode_oracle_price(&resp)
}


pub fn decode_oracle_price(data: &[u8])-> Result<U256,anyhow::Error> {
    if data.len() < 32 {
        return Err(anyhow::anyhow!("response too short"));
    }
    Ok(U256::from_be_slice(data))
}




