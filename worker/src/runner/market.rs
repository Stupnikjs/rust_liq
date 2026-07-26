use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use alloy_primitives::{Address, FixedBytes};
use eth_core::utils::BoxError;
use tokio::sync::RwLock;

use crate::backtest::{BacktestStore, SnapshotGroup, WriteBatch, build_groups};
use crate::cache::positions::BorrowPosition;
use crate::cache::{MarketCache, MarketSnapshot};
use crate::liquidate;
use crate::runner::Runner;
use crate::swap::now_ms;
use crate::swap::routes::RouteCache;
use connector::{Connector, rpc::CallStats};
use morpho::types::{price_normalized, MarketParam};

/// Nombre de cycles de la boucle marché entre deux refresh complets on-chain
/// (au lieu du seul oracle+HF, qui lui est rafraîchi à chaque cycle).
const THRESHOLD: u64 = 10;

/// Nombre max de tentatives de liquidation par position avant abandon,
/// pour éviter de spammer le mempool sur une position qui échoue en boucle.
const MAX_LIQUIDATION_ATTEMPTS: u16 = 20;

const BATCH_FLUSH_SIZE: usize = 32;

pub struct MarketLoopConsumer {
    liquidation_attempts: HashMap<Address, u16>,
    cache: Arc<MarketCache>,
    connector: Arc<Connector>,
    route_cache: Arc<RwLock<RouteCache>>,
    morpho_addr: Address,
    liquidator_addr: Address,
    id: FixedBytes<32>,
    backtest: Arc<BacktestStore>,
}

impl MarketLoopConsumer {
    /// `refresh_every` contrôle la fréquence (en nombre de cycles) du refresh
    /// complet du marché on-chain ; l'oracle et les health factors, eux, sont
    /// rafraîchis à chaque cycle quel que soit `refresh_every`.
    pub async fn run(mut self, refresh_every: u64) -> Result<(), BoxError> {
        let mut batch: Vec<SnapshotGroup> = Vec::with_capacity(BATCH_FLUSH_SIZE);
        let mut count: u64 = 0;
        let mut last_interval = 0;
        let mut tier: u8 = 1;

        loop {
            tier = next_tier(tier, last_interval);
            let call_stats_arr:Vec<CallStats> = Vec::new(); 
            // call stat (rpc_url, latency, type (oracle/market/liquidate) )
            // increment Vec<CallStat> => push in BacktestStore 
            if let Err(err) = self.refresh(count, refresh_every, tier).await {
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

            // interval == 0 signifie qu'une position est sous le seuil de liquidation immédiat.
            if let (Some(pos), 0) = (lowest, interval) {
                if let Err(err) = self.try_liquidate(pos, mparam).await {
                    eprintln!("[{:?}] liquidation attempt failed: {err:?}", self.id);
                }
            }

            if let Err(err) = self.batching(&snap, &mut batch, call_stats_arr).await {
                eprintln!("[{:?}] batching failed: {err:?}", self.id);
            }

            count += 1;
            tokio::time::sleep(Duration::from_secs(interval)).await;
        }
    }

    async fn try_liquidate(&mut self, pos: BorrowPosition, mparam: MarketParam) -> Result<(), BoxError> {
        let route = self.route_cache.read().await.get_edge(&pos.market_id).cloned();
        let Some(route) = route else { return Ok(()) };

        let attempts = self.liquidation_attempts.entry(pos.address).or_default();
        if *attempts >= MAX_LIQUIDATION_ATTEMPTS {
            return Ok(());
        }
        *attempts += 1;

        liquidate::liquidate(&self.connector, pos, route, mparam, self.liquidator_addr).await?;
        Ok(())
    }

    /// Rafraîchit l'oracle et recalcule les health factors.
    /// Retourne l'erreur au lieu de l'avaler, pour que l'appelant puisse la logger.
    async fn refresh_oracle_and_hf(&self, tier: u8) -> Result<CallStats, BoxError> {
        let start = now_ms(); 
        self.cache.onchain_oracle_refresh(&self.connector, tier, self.id).await?;
        let latency_ms =now_ms() - start; 
        self.cache.recompute_all_hf(self.id)?;
        Ok(CallStats{
          latency_ms,
          call_type: connector::rpc::CallType::OracleCall, 
        })
         
    }

    /// Rafraîchit l'état complet du marché on-chain et retrie par health factor.
    async fn market_refresh_and_sort(&self, tier: u8) -> Result<CallStats, BoxError> {
        let start = now_ms(); 
        self.cache
            .onchain_market_refresh(&self.connector, tier, self.morpho_addr, self.id)
            .await?;
        let latency_ms =now_ms() - start; 
        let _ = self.cache.sort_by_hf(self.id); 
         Ok(CallStats{
          latency_ms,
          call_type: connector::rpc::CallType::MarketCall, 
        })
    }

    async fn refresh(&self, count: u64, refresh_every: u64, tier: u8) -> Result<Vec<CallStats>, BoxError> {
        let mut stats: Vec<CallStats> = Vec::new(); 
        let oracle_call_stats = self.refresh_oracle_and_hf(tier).await?;
        stats.push(oracle_call_stats);
        if count % refresh_every == 0 {
            let market_call_stats = self.market_refresh_and_sort(tier).await?;
            stats.push(market_call_stats);
        }
         
        Ok(stats)
    }

    async fn batching(&mut self, snap: &MarketSnapshot, batch: &mut Vec<SnapshotGroup>, calls_stats: Vec<CallStats>) -> Result<(), BoxError> {
        let to_push = build_groups(snap, calls_stats);
        batch.extend_from_slice(&to_push);
        
        if batch.len() >= BATCH_FLUSH_SIZE {
            let groups = std::mem::take(batch);
            self.backtest.push_snapshot(WriteBatch { groups }).await?;
            batch.clear();
        }
        Ok(())
    }

    fn snapshot(&self) -> Option<(MarketSnapshot, MarketParam)> {
        let mparam = self.cache.get_market_param_by_id(self.id)?;
        let snap = self.cache.snapshot(self.id)?;
        Some((snap, mparam))
    }
}

/// Hystérésis simple : passe en tier "rapide" (0) dès que l'intervalle de
/// liquidation observé descend sous THRESHOLD, et repasse en tier "normal" (1)
/// une fois qu'il repasse au-dessus. Évite d'osciller à chaque cycle pile au seuil.
fn next_tier(current_tier: u8, last_interval: u64) -> u8 {
    match (current_tier, last_interval < THRESHOLD) {
        (1, true) => 0,
        (0, false) => 1,
        (t, _) => t,
    }
}

impl Runner {
    pub async fn market_loop(&self) -> Result<(), BoxError> {
        for (index, id) in self.cache.ids().into_iter().enumerate() {
            let consumer = MarketLoopConsumer {
                liquidation_attempts: HashMap::new(),
                cache: Arc::clone(&self.cache),
                connector: Arc::clone(&self.connector),
                route_cache: Arc::clone(&self.route_cache),
                morpho_addr: self.config.morpho_addr,
                liquidator_addr: self.config.liquidator_addr,
                backtest: Arc::clone(&self.backtest),
                id,
            };

            let refresh_every = (index + 1) as u64;
            tokio::spawn(consumer.run(refresh_every));
        }
        Ok(())
    }
}

pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

pub fn is_correlated(price_norm: f64, mparam: &MarketParam) -> bool {
    price_norm > 0.90 && price_norm < 1.1 || mparam.is_eth_correlated() || mparam.is_btc_correlated()
}

