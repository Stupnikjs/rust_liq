use std::time::Duration;
use morpho_api_graph::fetch_all_positions; 
use crate::cache::parse::{ position_item_to_borrow_pos}; 
use crate::cache::{MarketCache, BorrowPosition}; 
use crate::morpho::calls::{oracle_call, market_call}; 
use connector::Connector; 
use alloy_primitives::{Address, FixedBytes, U256};
use alloy::providers::Provider;
use alloy::network::Ethereum;
use futures::stream::{self, StreamExt};


impl MarketCache {
    pub async fn api_refresh(&self, chain_id: u32) {
    stream::iter(self.ids())
        .for_each_concurrent(5, |id| async move {
            match  fetch_all_positions(id, chain_id).await { //
                Ok(positions) if positions.len() > 10 => {
                    let borrow_pos_arr: Vec<BorrowPosition> = positions
                        .into_iter()
                        .map(|p| position_item_to_borrow_pos(p, id))
                        .filter(|p| p.borrow_assets_usd > 1.0)
                        .collect();
                    let max_collateral_pos = borrow_pos_arr
                        .iter()
                        .map(|p| p.collateral_assets)
                        .max()
                        .unwrap_or(U256::ZERO);
                    
                    self.update(id, |m| {
                        m.positions = borrow_pos_arr;
                        m.stats.max_collateral_pos = max_collateral_pos;
                    });
                }
                Ok(_) => {
                    self.update(id, |m| m.canceled = true);
                }
                Err(e) => {
                    eprintln!("fetch_all_positions failed for {:?}: {}", id, e);
                    self.update(id, |m| m.canceled = true); // ou un autre statut "stale"
                }
            }
        })
        .await;

    }
     pub async fn onchain_oracle_refresh(
        &self,
        conn: &Connector,
        market_id: FixedBytes<32>,
    ) -> Result<(), anyhow::Error> {
        let params = self.get_market_param_by_id(market_id)
            .ok_or(anyhow::anyhow!("market not found"))?;
         
       let price = oracle_call(conn, params.oracle).await?;

        self.update(market_id, |m| {
            m.stats.oracle_price = price;
        });

        Ok(())
    }

    pub async fn onchain_market_refresh(
        &self,
        conn: &Connector,
        morpho_addr:Address,
        market_id: FixedBytes<32>,
    ) -> Result<(), anyhow::Error> {
        let m_stats_result = market_call(conn, morpho_addr, market_id.as_slice()).await?;
        self.update(market_id, |m| {
            m.stats.total_borrow_assets = m_stats_result.total_borrow_assets;
            m.stats.total_borrow_shares = m_stats_result.total_borrow_shares;
        });

        Ok(())
    }
                
}
