use alloy::primitives::Address;
use alloy::signers::local::PrivateKeySigner;
use crate::swap;
use std::env::var;
use std::str::FromStr;
use std::sync::Arc;
use crate::runner;

mod address; 


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
    pub main_rpc: String,
    pub second_rpc: String,
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
pub fn load_base_config(slow_mode:bool) -> Result<Config, anyhow::Error> {
    dotenvy::dotenv().ok();
    let main_rpc = if slow_mode {
        var("CHAINSTACK_BASE_HTTP").expect("CHAINSTACK_BASE_HTTP not set")
    } else { var("ALCHEMY_BASE_HTTP").expect("CHAINSTACK_BASE_HTTP not set") }; 

    Ok(Config{ 
        chain_id: 8453,
        main_rpc:main_rpc,  // var("BASE_HTTP_DRPC").expect("BASE_HTTP_DRPC not set") ,
        second_rpc: var("DRPC_BASE_HTTP").expect("DRPC_BASE_HTTP not set"),
        ws_rpc: var("ALCHEMY_BASE_WS").expect("ALCHEMY_BASE_WS not set"),
        morpho_addr: runner::config::address::MORPHO_MAINNET,
        liquidator_addr: runner::config::address::BASE_LIQUIDATOR_LAST,
        dexes: vec![new_dex_config(address::BASE_UNISWAP_QUOTER_V2, address::BASE_UNISWAP_V3_ROUTER, DexesName::UniswapV3)] ,
        signer: PrivateKeySigner::from_str(&var("MY_SAFE_PK").expect("PK NOT SET"))?,
       
    })
}

pub fn load_arb_config(slow_mode:bool) -> Result<Config, anyhow::Error> {
   dotenvy::dotenv().ok();
     
    let main_rpc = if slow_mode {
        println!("slow mode on "); 
        var("DRPC_ARB_HTTP").expect("DRPC_ARB_HTTP not set")
    } else { var("ALCHEMY_BASE_HTTP").expect("ALCHEMY_BASE_HTTP not set") }; 
    let sec_rpc = if slow_mode {
        var("ANKR_ARB_HTTP").expect("ANKR_ARB_HTTP not set")
    } else { var("DRPC_ARB_HTTP").expect("DRPC_ARB_HTTP not set") }; 

    Ok(Config {
        chain_id: 42161,
        main_rpc:main_rpc,
        second_rpc: sec_rpc,
        ws_rpc: var("ALCHEMY_ARB_WS").expect("ALCHEMY_ARB_WS not set"),
        morpho_addr: runner::config::address::ARBITRUM_MORPHO,
        liquidator_addr: runner::config::address::ARBITRUM_LIQUIDATOR,
        dexes: vec![new_dex_config(address::ARBITRUM_UNISWAP_QUOTER_V2, address::ARBITRUM_UNISWAP_V3_ROUTER, DexesName::UniswapV3)] ,
        signer: PrivateKeySigner::from_str(&var("MY_SAFE_PK").expect("PK NOT SET"))?,
        
    })
}


pub fn load_katana_config(slow_mode:bool) -> Result<Config, anyhow::Error> {
   dotenvy::dotenv().ok();
    let main_rpc = if slow_mode {
        var("DRPC_KATANA_HTTP").expect("DRPC_KATANA_HTTP not set")
    } else { var("ALCHEMY_BASE_HTTP").expect("ALCHEMY_BASE_HTTP not set") }; 
    let sec_rpc = if slow_mode {
        var("PUB_KATANA_HTTP").expect("ANKR_ARB_HTTP not set")
    } else { var("DRPC_KATANA_HTTP").expect("DRPC_KATANA_HTTP not set") }; 

    Ok(Config {
        chain_id: 747474,
        main_rpc:main_rpc,
        second_rpc: sec_rpc,
        ws_rpc: var("ALCHEMY_KATANA_WS").expect("ALCHEMY_KATANA_WS not set"),
        morpho_addr: runner::config::address::KATANA_MORPHO,
        liquidator_addr: runner::config::address::KATANA_LIQUIDATOR,
        dexes: vec![new_dex_config(address::KATANA_UNISWAP_QUOTER_V2, address::KATANA_UNISWAP_V3_ROUTER, DexesName::UniswapV3)] ,
        signer: PrivateKeySigner::from_str(&var("MY_SAFE_PK").expect("PK NOT SET"))?,
        
    })
}