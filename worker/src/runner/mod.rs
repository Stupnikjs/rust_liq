#[warn(unreachable_code)]
use std::collections::HashMap;
use std::error::Error;
use std::sync::{Arc};
use tokio::sync::RwLock; 
use crate::config::{Config, load_base_config};
use connector::{Connector};
use eth_core::traits::RpcKind; 
use crate::cache::{MarketCache, logs::MarketLog, parse::fetch_parse_all_market};
//use crate::runner::config::load_katana_config;
use morpho::types::MarketParam;

use crate::backtest::{BacktestStore, BacktestSnapshot};
use crate::runner::{server::build_router}; 
use crate::swap::routes::RouteCache;


mod api; 
mod market;
mod server;
mod quote;


pub const EVENTS_SIG: [&str; 7] = [
            "Supply(bytes32,address,address,uint256,uint256)",
            "Borrow(bytes32,address,address,address,uint256,uint256)",
            "Repay(bytes32,address,address,uint256,uint256)",
            "Liquidate(bytes32,address,address,uint256,uint256,uint256,uint256,uint256)",
            "AccrueInterest(bytes32,uint256,uint256,uint256)",
            "SupplyCollateral(bytes32,address,address,uint256)",
            "WithdrawCollateral(bytes32,address,address,address,uint256)",
]; 

pub struct Runner {
    config: Arc<Config>,
    cache: Arc<MarketCache>,
    connector: Arc<Connector>,
    route_cache: Arc<RwLock<RouteCache>>,
    log_store: Arc<RwLock<HashMap<String, MarketLog>>>,
    backtest: Arc<BacktestStore>,
}

impl Runner {
    pub async fn new(chainid: u64) -> Result<Runner, Box<dyn Error>> {
        let config = match chainid {
            8453 => load_base_config()?,
           // 42161 => load_arb_config(slow_mode)?,
           // 747474 => load_katana_config(slow_mode)?,
            _ => panic!("unsupported chain {}", chainid),
        };

        let config = Arc::new(config);
        let cache = Arc::new(MarketCache::new(&[]));
        let rpc_configs = config.rpc_configs.clone(); 
        let conn = connector::build(rpc_configs, &config.ws_rpc, config.signer.clone(), chainid).await?;
        let connector = Arc::new(conn);
        
        let route_cache = Arc::new(RwLock::new(RouteCache::new()));
        let log_store = Arc::new(RwLock::new(HashMap::new()));

        // Error lauching db 
        let backtest = Arc::new(BacktestStore::new("worker/data/db").await?);

        Ok(Self { config, cache, connector, route_cache, log_store, backtest })
    }
    


    pub async fn init(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let markets:Vec<MarketParam> = fetch_parse_all_market(self.config.chain_id).await?;
        self.cache = Arc::new(MarketCache::new(&markets));
         
        self.cache.api_refresh(self.config.chain_id).await;

        // geting oracle prices 
        for market_id in self.cache.ids() {
            let _ = self.cache.onchain_oracle_refresh(self.connector.as_ref(),RpcKind::Secondary, market_id, ).await;
            let _ = self.cache.onchain_market_refresh(self.connector.as_ref(), RpcKind::Secondary, self.config.morpho_addr, market_id).await;
            self.cache.recompute_all_hf(market_id);  
            self.cache.sort_by_hf(market_id);
        }

        self.quote_market().await; 
        println!("init done"); 

        Ok(())
    }

    pub async fn run(self: Arc<Self>) -> Result<(), Box<dyn std::error::Error>> {
    let sub_handle = {
        let this = Arc::clone(&self);
        tokio::spawn(async move {
            loop {
                let cache = this.cache.clone();
                if let Err(e) = this
                    .connector
                    .subscribe(this.config.morpho_addr, &EVENTS_SIG, move |log| {
                        cache.process_log(&log); 
                                            
                                         
                    })
                    .await
                {
                    eprintln!("subscribe task failed: {e}, reconnecting in 2s");
                }
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            }
        })
    };

    let refresh_handle = {
        let this = Arc::clone(&self);
        tokio::spawn(async move {
            this.api_refresh_loop(7200).await;
        })
    };

    let market_handle = {
        let this = Arc::clone(&self);
        tokio::spawn(async move {
            print!("spawning markets"); 
            this.market_loop().await;
        })
    };

        // nouveau: serveur axum, même niveau que les autres
    let log_handle = {
    let cache = Arc::clone(&self.cache);
    let backtest_store = Arc::clone(&self.backtest); 
    tokio::spawn(async move {
        let app = build_router(cache, backtest_store);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:9090")
            .await
            .expect("failed to bind API port");
        if let Err(e) = axum::serve(listener, app).await {
            eprintln!("API server failed: {e}");
        }
    })
};

      let (log_res, market_res, refresh_res, sub_res) = tokio::join!(
        log_handle,
        market_handle,
        refresh_handle,
        sub_handle,
    );

    if let Err(e) = log_res {
        eprintln!("log_handle (axum server) task panicked: {e}");
    }
    if let Err(e) = market_res {
        eprintln!("market_handle task panicked: {e}");
    }
    if let Err(e) = refresh_res {
        eprintln!("refresh_handle task panicked: {e}");
    }
    if let Err(e) = sub_res {
        eprintln!("sub_handle task panicked: {e}");
    }

    Ok(())
}
} 





 

