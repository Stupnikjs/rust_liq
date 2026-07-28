use alloy::{primitives::Address, providers::RootProvider};
use alloy::signers::local::PrivateKeySigner;
use crate::swap;
use core::time;
use std::env::var;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration; 
use crate::runner;
use connector::rpc::{Tier, RpcEndpoint}; 


mod address; 
pub mod json; 


pub struct DexConfig {
    pub quoter: Address,
    pub router: Address,
    pub name: DexesName,
}

pub enum DexesName {
    UniswapV3,
    Pankake, 
    Aerodrome,  
}

pub struct Config {
    pub chain_id: u32,
    pub rpc_configs: Vec<Arc<RpcEndpoint>>,
    pub ws_rpc: String,
    pub morpho_addr: Address,
    pub liquidator_addr: Address,
    pub dexes: Vec<DexConfig>,
    pub signer: PrivateKeySigner, 
}


pub fn new_dex_config(quoter: Address, router: Address, name: DexesName) -> DexConfig {
    DexConfig {
        quoter,
        router,
        name: name,
    }
}



     