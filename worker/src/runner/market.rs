use std::collections::HashMap;
use std::sync::{Arc};
use alloy_primitives::{Address,FixedBytes};
use connector::rpc::Tier::Garbage;
use eth_core::utils::BoxError;
use tokio::sync::RwLock; 
use crate::backtest::{BacktestSnapshot, BacktestStore, snap_to_4_batch};
use crate::cache::positions::BorrowPosition;
use crate::cache::{ MarketCache, MarketSnapshot};
use connector::{Connector, rpc::Tier};
use crate::swap::routes::RouteCache;
use crate::{liquidate}; 
use morpho::types::{MarketParam, price_normalized}; 
use crate::runner::Runner; 
use std::time::{SystemTime, UNIX_EPOCH, Duration};


const THRESHOLD:u64 = 10; 

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
    pub async fn run(mut self, index: u64) -> Result<(), BoxError> {
        let mut batch: Vec<BacktestSnapshot> = Vec::with_capacity(32);
        let mut count: u64 = 0;
        let mut last_interval = 0;
        let mut tier: u8 = 1;

        loop {
            if last_interval < THRESHOLD && tier == 1 {
                tier = 0;
            } else if last_interval >= THRESHOLD && tier == 0 {
                tier = 1;
            }

            if let Err(err) = self.refresh(count, index, tier).await {
                eprintln!("[{:?}] refresh failed: {err:?}", self.id);
            }

            let Some((snap, mparam)) = self.snapshot() else {
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
            last_interval = interval;

            if let (Some(pos), 0) = (lowest, interval) {
                if let Err(err) = self.try_liquidate(pos, mparam).await {
                    eprintln!("[{:?}] liquidation attempt failed: {err:?}", self.id);
                }
            }

            if let Err(err) = self.batching(&snap, &mut batch).await {
                eprintln!("[{:?}] batching failed: {err:?}", self.id);
            }

            count += 1;
            tokio::time::sleep(Duration::from_secs(interval)).await;
        }
    }



    async fn try_liquidate(&mut self, pos:BorrowPosition, mparam:MarketParam) -> Result<(), BoxError>  {

        let route = self.route_cache.read().await.get_edge(&pos.market_id).cloned();
        let Some(route) = route else { return Ok(()) };

        let attempts = self.spaming_map.entry(pos.address).or_default();
        if *attempts < 20 {
            *attempts += 1;
            let _ = liquidate::liquidate(&self.connector, pos, route, mparam.clone(), self.liquidator_addr).await;
            
        }
        Ok(())

    }

    async fn refresh_oracle_and_hf(&self, tier:u8) -> Result<(), BoxError> {
        let _ = self.cache.onchain_oracle_refresh(&self.connector, tier, self.id).await;
        let _ = self.cache.recompute_all_hf(self.id);
        Ok(())
    }

    async fn market_refresh_and_sort(&self, tier:u8) -> Result<(), BoxError> {
        
            let _ = self
            .cache
            .onchain_market_refresh(&self.connector, tier, self.morpho_addr, self.id)
            .await;
        
        
        self.cache.sort_by_hf(self.id)
    }

    async fn refresh(&self, count: u64, refresh_every: u64, tier:u8) -> Result<(), BoxError> {
    let _ = self.refresh_oracle_and_hf(tier).await;

    if count % refresh_every == 0 {
        let _ = self.market_refresh_and_sort(tier).await;
    }
    Ok(())
}

    async fn batching(& mut self, snap:&MarketSnapshot, batch: &mut Vec<BacktestSnapshot>,) -> Result<(), BoxError>{
            let to_push_in_batch = snap_to_4_batch(snap);
            batch.extend_from_slice(to_push_in_batch.as_slice());
            if batch.len() >= 32 {
                let _ = self.backest.push_snapshot(batch).await;
                batch.clear();
            } 
            Ok(())
    }
        fn snapshot(
    &self,
) -> Option<(MarketSnapshot, MarketParam)> {
    let mparam = self.cache.get_market_param_by_id(self.id)?;
    let snap = self.cache.snapshot(self.id)?;

    Some((snap, mparam))
}
}

impl Runner {
    pub async fn market_loop(&self) -> Result<(), BoxError>{
        for (index, id) in self.cache.ids().into_iter().enumerate() {
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
             
            tokio::spawn(lc.run((index + 1) as u64));
            
        }
        Ok(())
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
