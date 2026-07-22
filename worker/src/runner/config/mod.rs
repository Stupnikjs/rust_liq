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
}pub fn load_base_config() -> anyhow::Result<Config> {
    dotenvy::dotenv().ok();

    let mut rpc_config: Vec<Arc<RpcEndpoint>> = Vec::new();

    // RPC privés / premium
    rpc_config.push(Arc::new(
        RpcEndpoint::new(
            var("DRPC_BASE_HTTP")?,
            Tier::Top,
            Duration::from_millis(200),
        )?,
    ));

    rpc_config.push(Arc::new(
        RpcEndpoint::new(
            var("CHAINSTACK_BASE_HTTP")?,
            Tier::Top,
            Duration::from_millis(200),
        )?,
    ));

    // RPC publics
    rpc_config.push(Arc::new(
        RpcEndpoint::new(
            "https://rpc.baseazul.dev".to_string(),
            Tier::Garbage,
            Duration::from_secs(20),
        )?,
    ));

    rpc_config.push(Arc::new(
        RpcEndpoint::new(
            "https://base-rpc.publicnode.com".to_string(),
            Tier::Garbage,
            Duration::from_secs(20),
        )?,
    ));

    rpc_config.push(Arc::new(
        RpcEndpoint::new(
            "https://base-mainnet.gateway.tatum.io".to_string(),
            Tier::Garbage,
            Duration::from_secs(20),
        )?,
    ));

    rpc_config.push(Arc::new(
        RpcEndpoint::new(
            "https://base.drpc.org".to_string(),
            Tier::Garbage,
            Duration::from_secs(20),
        )?,
    ));

    rpc_config.push(Arc::new(
        RpcEndpoint::new(
            "https://rpc.nodeflare.app/base/public".to_string(),
            Tier::Garbage,
            Duration::from_secs(20),
        )?,
    ));

    rpc_config.push(Arc::new(
        RpcEndpoint::new(
            "https://base.public.blockpi.network/v1/rpc/public".to_string(),
            Tier::Garbage,
            Duration::from_secs(20),
        )?,
    ));

    rpc_config.push(Arc::new(
        RpcEndpoint::new(
            "https://base.lava.build".to_string(),
            Tier::Garbage,
            Duration::from_secs(20),
        )?,
    ));

    rpc_config.push(Arc::new(
        RpcEndpoint::new(
            "https://base-rpc.keccak.io".to_string(),
            Tier::Garbage,
            Duration::from_secs(20),
        )?,
    ));


    rpc_config.push(Arc::new(
    RpcEndpoint::new(
        "https://api.zan.top/base-mainnet".to_string(),
        Tier::Garbage,
        Duration::from_secs(20),
    )?,
));

    rpc_config.push(Arc::new(
        RpcEndpoint::new(
            "https://gateway.tenderly.co/public/base".to_string(),
            Tier::Garbage,
            Duration::from_secs(20),
        )?,
    ));

    rpc_config.push(Arc::new(
        RpcEndpoint::new(
            "https://base.gateway.tenderly.co".to_string(),
            Tier::Garbage,
            Duration::from_secs(20),
        )?,
    ));

    rpc_config.push(Arc::new(
        RpcEndpoint::new(
            "https://base-mainnet.public.blastapi.io".to_string(),
            Tier::Garbage,
            Duration::from_secs(20),
        )?,
    ));

    rpc_config.push(Arc::new(
        RpcEndpoint::new(
            "https://base.rpc.blxrbdn.com".to_string(),
            Tier::Garbage,
            Duration::from_secs(20),
        )?,
    ));

    rpc_config.push(Arc::new(
        RpcEndpoint::new(
            "https://base-public.nodies.app".to_string(),
            Tier::Garbage,
            Duration::from_secs(20),
        )?,
    ));

    rpc_config.push(Arc::new(
        RpcEndpoint::new(
            "https://base.meowrpc.com".to_string(),
            Tier::Garbage,
            Duration::from_secs(20),
        )?,
    ));

    rpc_config.push(Arc::new(
        RpcEndpoint::new(
            "https://mainnet.base.org".to_string(),
            Tier::Garbage,
            Duration::from_secs(20),
        )?,
    ));

    rpc_config.push(Arc::new(
        RpcEndpoint::new(
            "https://developer-access-mainnet.base.org".to_string(),
            Tier::Garbage,
            Duration::from_secs(20),
        )?,
    ));

    rpc_config.push(Arc::new(
        RpcEndpoint::new(
            "https://base.rpc.sentio.xyz".to_string(),
            Tier::Garbage,
            Duration::from_secs(20),
        )?,
    ));
    Ok(Config {
        chain_id: 8453,
        rpc_configs: rpc_config,
        ws_rpc: var("DRPC_BASE_WS")?,
        morpho_addr: runner::config::address::MORPHO_MAINNET,
        liquidator_addr: runner::config::address::BASE_LIQUIDATOR_LAST,
        dexes: vec![
            new_dex_config(
                address::BASE_UNISWAP_QUOTER_V2,
                address::BASE_UNISWAP_V3_ROUTER,
                DexesName::UniswapV3,
            )
        ],
        signer: PrivateKeySigner::from_str(&var("MY_SAFE_PK")?)?,
    })
}

/* 
pub fn load_arb_config(slow_mode:bool) -> Result<Config, anyhow::Error> {
   dotenvy::dotenv().ok();
     
    let main_rpc = if slow_mode {
        println!("slow mode on"); 
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
}*/