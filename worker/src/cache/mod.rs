#![allow(dead_code, unused_variables, unused_imports)]
mod refresh;
pub mod positions;
mod events;
mod sort; 
pub mod logs;
pub mod parse;

use alloy_primitives::{Address, U256, FixedBytes};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use crate::morpho::types::MarketParam;
use crate::morpho::utils::WAD; 
use crate::cache::parse::{fetch_parse_all_market}; 
use morpho_api_graph::fetch_all_positions; 

use positions::BorrowPosition;
use futures::stream::{self, StreamExt};




#[derive(Default, Clone)]
pub struct MarketStats {
    pub total_borrow_assets: U256,
    pub total_borrow_shares: U256,
    pub borrow_rate: U256,
    pub max_collateral_pos: U256,
    pub oracle_price: U256,
}

pub struct Market {
    pub params: Arc<MarketParam>,
    pub canceled: bool,
    pub stats: MarketStats,
    pub positions: Vec<BorrowPosition>, // trié par HF asc
}

pub struct MarketSnapshot {
    pub id: FixedBytes<32>,
    pub params: Arc<MarketParam>,
    pub stats: MarketStats,
    pub positions: Vec<BorrowPosition>,
}

pub struct MarketCache {
    markets: RwLock<HashMap<FixedBytes<32>, Arc<RwLock<Market>>>>,
}


impl MarketCache {
    pub fn new(markets: &[MarketParam]) -> Self {
        let map: HashMap<FixedBytes<32>, Arc<RwLock<Market>>> = markets
            .iter()
            .map(|m| {
                // On copie et transfère les vrais paramètres ici !
                let market = Market {
                    params: Arc::new(m.clone()),
                    canceled: false,
                    stats: MarketStats::default(), // Les stats globales (volumes) seront mis à jour par l'indexeur
                    positions: Vec::new(), // Le tableau de positions démarre vide
                };
                
                let id_bytes: FixedBytes<32> = m.id.into(); 
                (id_bytes, Arc::new(RwLock::new(market)))
            })
            .collect();

        Self { markets: RwLock::new(map) }
    }

    pub fn ids(&self) -> Vec<FixedBytes<32>> {
        self.markets
            .read()
            .unwrap()
            .iter()
            .filter(|(_, market)| !market.read().unwrap().canceled)
            // On prend le premier élément du tuple (la clé), et on ignore le second avec `_`
            .map(|(&id_bytes, _market)| FixedBytes::from(id_bytes)) 
            .collect()
    }

    pub fn get_market_param_by_id(&self, id: FixedBytes<32>,) -> Option<MarketParam> {
    self.markets
        .read()
        .unwrap()
        .get(&id)
        .map(|m| (*m.read().unwrap().params).clone())
} 

    pub fn update<F, R>(&self, id:FixedBytes<32>, f: F) -> Option<R>
where
    F: FnOnce(&mut Market) -> R,
{
    // 1. Lock la HashMap juste le temps de récupérer l'Arc, puis relâche immédiatement
    let market_arc = {
        let guard = self.markets.read().unwrap();
        guard.get(&id)?.clone()
    };

    // 2. Lock le Market séparément — gère le poisoning sans paniquer
    let mut market = match market_arc.write() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(), // récupère quand même les données
    };

    Some(f(&mut market))
}

    pub fn snapshot(&self, id: FixedBytes<32>,) -> Option<MarketSnapshot>
    where
        BorrowPosition: Clone,
    {
        let guard = self.markets.read().unwrap();
        let market = guard.get(&id)?.read().unwrap();
        if market.positions.len() == 0 { 
            return None;
        }
        Some(MarketSnapshot {
            params: market.params.clone(),
            id,
            stats: market.stats.clone(),
            positions: market.positions.clone(),
        })
    }

    // cached_hf must be set 
  pub fn insert_pos(&self, pos: &BorrowPosition) {
    let snap = self.snapshot(pos.market_id).expect("snap in insert_pos failed");

    let mut new_positions: Vec<BorrowPosition> = Vec::with_capacity(snap.positions.len() + 1);

    if pos.cached_hf.is_none() {
        new_positions.extend_from_slice(&snap.positions);
        new_positions.push(pos.clone());
        let _ = self.update(pos.market_id, |m| m.positions = new_positions);
        return;
    }

    let insert_at = snap
        .positions
        .iter()
        .position(|p| p.cached_hf >= pos.cached_hf)
        .unwrap_or(snap.positions.len()); // append si le plus grand

    new_positions.extend_from_slice(&snap.positions[..insert_at]);
    new_positions.push(pos.clone());
    new_positions.extend_from_slice(&snap.positions[insert_at..]);

    let _ = self.update(pos.market_id, |m| m.positions = new_positions);
}

  pub fn remove_pos(&self, pos: &BorrowPosition) {
    let snap = self.snapshot(pos.market_id).expect("snap in remove_pos failed");

    let new_positions: Vec<BorrowPosition> = snap
        .positions
        .into_iter()
        .filter(|p| p.address != pos.address)
        .collect();

    let _ = self.update(pos.market_id, |m| m.positions = new_positions);
}

  pub fn all_snapshots(&self) -> HashMap<FixedBytes<32>, MarketSnapshot> {
    
    let mut map = HashMap::new(); 
    for id in self.ids() {
       let snap = self.snapshot(id).expect("snapshot failed");
       map.insert(id, snap);  

    }
    map
  } 
}



