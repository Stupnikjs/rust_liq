use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use alloy_primitives::{Address,FixedBytes}; 
use crate::backtest::{BacktestSnapshot, BacktestStore, snap_to_4_batch};
use crate::cache::positions::BorrowPosition;
use crate::cache::{ MarketCache};
use connector::Connector;
use crate::swap::routes::RouteCache;
use crate::{liquidate, morpho}; 
use crate::morpho::types::{MarketParam, price_normalized}; 
use crate::runner::Runner; 
use std::time::{SystemTime, UNIX_EPOCH, Duration};

pub struct MarketLoopConsumer {
    spaming_map: HashMap<Address, u16>,
    cache: Arc<MarketCache>,
    connector: Arc<Connector>,
    route_cache: Arc<RwLock<RouteCache>>, 
    morpho_addr: Address,
    liquidator_addr: Address,
    id: FixedBytes<32>,
    backest:Arc<BacktestStore>,
}

impl MarketLoopConsumer {
    pub async fn run(mut self) {
        let mut count: u64 = 0;
        let mut last_sec = now_secs(); 
        let mut batch:Vec<BacktestSnapshot> = Vec::with_capacity(32);     

        loop {
            let now = now_secs(); 

            self.refresh_oracle_and_hf().await; 
            if count % 10 == 0 {
                self.market_refresh_and_sort().await; 
            }; 

            let Some(mparam) = self.cache.get_market_param_by_id(self.id) else {
                eprintln!("[market_loop] mparam introuvable pour market_id={:?}, skip ce tick", self.id);
                tokio::time::sleep(Duration::from_secs(3600)).await;
                continue;
            };
            let Some(snap) = self.cache.snapshot(self.id) else {
                eprintln!("[market_loop] snapshot introuvable pour market_id={:?}, skip ce tick", self.id);
                tokio::time::sleep(Duration::from_secs(3600)).await;
                continue;
            };
            let price_norm = price_normalized(
                mparam.loan_token_decimals,
                mparam.collateral_token_decimals,
                snap.stats.oracle_price,
            );
            let is_correlated = is_correlated(price_norm, &mparam); 
            let (lowest, interval) = self.cache.lowest_hf_and_interval(self.id, is_correlated);
            if let (Some(pos), 0) = (lowest, interval) {
                self.try_liquidate(pos, mparam).await; 
            }

            let to_push_in_batch = snap_to_4_batch(&snap);
            batch.extend_from_slice(to_push_in_batch.as_slice());
            if batch.len() >= 32 {
                let _ = self.backest.push_snapshot(&batch).await;
                batch.clear();
            }
            
            if now - last_sec > 100 {
                last_sec = now; 
            }
            count += 1;
            tokio::time::sleep(Duration::from_secs(interval)).await;
        }
    }


    async fn try_liquidate(&mut self, pos:BorrowPosition, mparam:MarketParam) {
        let route = self.route_cache.read().unwrap().get_edge(&pos.market_id).cloned();
        let Some(route) = route else { return };

        let attempts = self.spaming_map.entry(pos.address).or_default();
        if *attempts < 20 {
            *attempts += 1;
            liquidate::liquidate(&self.connector, pos, route, mparam.clone(), self.liquidator_addr).await;
            
        }
    }

    async fn refresh_oracle_and_hf(&self) {
        let _ = self.cache.onchain_oracle_refresh(&self.connector, self.id).await;
        self.cache.recompute_all_hf(self.id);
    }

    async fn market_refresh_and_sort(&self) {
        let _ = self
            .cache
            .onchain_market_refresh(&self.connector, self.morpho_addr, self.id)
            .await;
        self.cache.sort_by_hf(self.id);
    }
}

impl Runner {
    pub async fn market_loop(&self) {
        for id in self.cache.ids() {
            let lc = MarketLoopConsumer {
                spaming_map: HashMap::new(),
                cache: Arc::clone(&self.cache),
                connector: Arc::clone(&self.connector),
                route_cache: Arc::clone(&self.route_cache),
                morpho_addr: self.config.morpho_addr,
                liquidator_addr: self.config.liquidator_addr,
                backest: Arc::clone(&self.backtest),
                id,
            };

            tokio::spawn(lc.run());
        }
    }

    
}




pub fn now_secs() -> u64 {
      SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() // u64
}


    pub fn is_correlated(price_norm:f64, mparam:&MarketParam) -> bool {
        price_norm > 0.90 && price_norm < 1.1 || mparam.is_eth_correlated() || mparam.is_btc_correlated()
    }
