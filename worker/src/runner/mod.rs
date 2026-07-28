mod api;
mod market;
mod server;
mod quote;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
 use alloy_primitives::FixedBytes; 

use eth_core::utils::BoxError;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;

use crate::config::{Config, json::{load_arb_config, load_base_config, load_katana_config}};
use connector::Connector;
use crate::cache::{MarketCache, logs::MarketLog, parse::fetch_parse_all_market};
use morpho::types::{MarketParam};

use crate::backtest::BacktestStore;
use crate::runner::server::build_router;
use crate::swap::routes::RouteCache;

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
    pub async fn new(chainid: u64) -> Result<Runner, BoxError> {
        let config_path = format!("./json_config/{chainid}/static.json");
        let config = match chainid {
            8453 => load_base_config(&config_path)?,
            42161 => load_arb_config(&config_path)?,
            747474 => load_katana_config(&config_path)?,
            _ => panic!("unsupported chain {}", chainid),
        };

        let config = Arc::new(config);
        let cache = Arc::new(MarketCache::new(&[]));
        let rpc_configs = config.rpc_configs.clone();
        let conn = connector::build(rpc_configs, &config.ws_rpc, config.signer.clone(), chainid).await?;
        let connector = Arc::new(conn);

        let route_cache = Arc::new(RwLock::new(RouteCache::new()));
        let log_store = Arc::new(RwLock::new(HashMap::new()));

        // BacktestStore::new peut échouer si le fichier db est corrompu ou verrouillé
        // par un autre process (ex: instance précédente pas proprement arrêtée).
        let backtest = Arc::new(BacktestStore::new(&format!("worker/data/{chainid}/db")).await?);

        Ok(Self { config, cache, connector, route_cache, log_store, backtest })
    }

    pub async fn init(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let markets: Vec<MarketParam> = fetch_parse_all_market(self.config.chain_id).await?;
        self.cache = Arc::new(MarketCache::new(&markets));
        self.cache.api_refresh(self.config.chain_id).await;

        for market_id in self.cache.ids() {
            self.refresh_market(market_id).await;
        }

        self.quote_market().await;
        println!("init done");

        Ok(())
    }

    /// Rafraîchit l'oracle et le marché on-chain pour un market_id donné,
    /// puis recalcule et retrie les health factors.
    async fn refresh_market(&self, market_id:alloy_primitives::FixedBytes<32> ) {
        if let Err(e) = self.cache.onchain_oracle_refresh(self.connector.as_ref(), 1, market_id).await {
           // eprintln!("oracle_refresh failed for {market_id}: {e}");
        }
        if let Err(e) = self.cache
            .onchain_market_refresh(self.connector.as_ref(), 1, self.config.morpho_addr, market_id)
            .await
        {
           // eprintln!("market_refresh failed for {market_id}: {e}");
        }
        let _ = self.cache.recompute_all_hf(market_id);
        let _ = self.cache.sort_by_hf(market_id);
    }

    pub async fn run(self: Arc<Self>) -> Result<(), Box<dyn std::error::Error>> {
        let handles: Vec<(&str, JoinHandle<()>)> = vec![
            ("subscription", tokio::spawn(self.clone().subscription_loop())),
            ("refresh", tokio::spawn(self.clone().refresh_loop(7200))),
            ("market", tokio::spawn(self.clone().market_loop_task())),
            ("api_server", tokio::spawn(self.clone().serve_api())),
        ];

        for (name, handle) in handles {
            if let Err(e) = handle.await {
                eprintln!("{name} task panicked: {e}");
            }
        }

        Ok(())
    }

    /// Écoute les logs on-chain en continu ; se reconnecte automatiquement
    /// toutes les 2s en cas d'échec de la souscription.
    async fn subscription_loop(self: Arc<Self>) {
        loop {
            let cache = self.cache.clone();
            if let Err(e) = self.connector
                .subscribe(self.config.morpho_addr, &EVENTS_SIG, move |log| {
                    cache.process_log(&log);
                })
                .await
            {
                eprintln!("subscribe task failed: {e}, reconnecting in 2s");
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    }

    async fn refresh_loop(self: Arc<Self>, interval_secs: u64) {
        self.api_refresh_loop(interval_secs).await;
    }

    async fn market_loop_task(self: Arc<Self>) {
        println!("spawning markets");
        let _ = self.market_loop().await;
    }

    /// Démarre le serveur HTTP Axum exposant l'API sur le port dédié à la chaîne.
    async fn serve_api(self: Arc<Self>) {
        let port = api_port(self.config.chain_id);
        let app = build_router(self.cache.clone(), self.backtest.clone(), self.connector.clone());

        let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{port}"))
            .await
            .expect("failed to bind API port");

        if let Err(e) = axum::serve(listener, app).await {
            eprintln!("API server failed: {e}");
        }
    }
}

/// Mappe un chain_id vers le port sur lequel exposer l'API locale.
fn api_port(chain_id: u32) -> u16 {
    match chain_id {
        8453 => 8453,
        747474 => 7474,
        42161 => 4216,
        _ => 0,
    }
}