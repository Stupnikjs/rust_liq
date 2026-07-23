use alloy::primitives::Address;
use alloy::signers::local::PrivateKeySigner;
use connector::rpc::{RpcEndpoint, Tier};
use serde::Deserialize;
use crate::config::{Config, DexConfig, DexesName, new_dex_config}; 
use std::env::var;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

#[derive(Deserialize, Debug)]
struct RawPublicRpc {
    url: String,
    timeout_secs: u64,
}

#[derive(Deserialize, Debug)]
struct RawDex {
    quoter: String,
    router: String,
    name: String,
}

#[derive(Deserialize, Debug)]
struct RawConfig {
    chain_id: u32,
    morpho_addr: String,
    liquidator_addr: String,
    premium_rpc_env_vars: Vec<String>,
    premium_timeout_ms: u64,
    public_rpcs: Vec<RawPublicRpc>,
    dexes: Vec<RawDex>,
}

pub fn load_base_config(config_path: &str) -> anyhow::Result<Config> {
    dotenvy::dotenv().ok();

    let raw: RawConfig = serde_json::from_str(&std::fs::read_to_string(config_path)?)?;
    let mut rpc_config: Vec<Arc<RpcEndpoint>> = Vec::new();

    // Endpoints premium : l'URL vient de l'env (contient souvent une clé API)
    let premium_timeout = Duration::from_millis(raw.premium_timeout_ms);
    for env_key in &raw.premium_rpc_env_vars {
        rpc_config.push(Arc::new(RpcEndpoint::new(
            var(env_key)?,
            Tier::Top,
            premium_timeout,
        )?));
    }

    // Endpoints publics : URL en clair dans le JSON, pas de secret
    for public in &raw.public_rpcs {
        rpc_config.push(Arc::new(RpcEndpoint::new(
            public.url.clone(),
            Tier::Garbage,
            Duration::from_secs(public.timeout_secs),
        )?));
    }

    let dexes = raw
        .dexes
        .into_iter()
        .map(|d| {
            let name = match d.name.as_str() {
                "UniswapV3" => DexesName::UniswapV3,
                "Pankake" => DexesName::Pankake,
                "Aerodrome" => DexesName::Aerodrome,
                other => anyhow::bail!("dex inconnu dans la config: {other}"),
            };
            Ok(new_dex_config(
                Address::from_str(&d.quoter)?,
                Address::from_str(&d.router)?,
                name,
            ))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

     
    Ok(Config {
        chain_id: raw.chain_id,
        rpc_configs: rpc_config,
        ws_rpc: var("DRPC_BASE_WS")?,
        morpho_addr: Address::from_str(&raw.morpho_addr)?,
        liquidator_addr: Address::from_str(&raw.liquidator_addr)?,
        dexes,
        signer: PrivateKeySigner::from_str(&var("MY_SAFE_PK")?)?,
    })
}