use crate::runner::{Runner}; 
use crate::{liquidate};
use std::collections::HashMap;
use std::fs;
use std::sync::Arc;
use std::str::FromStr;
use morpho::types::price_normalized; 
use alloy_primitives::Address;
use tokio::time::Duration;
use crate::swap::quoter::UniswapV3;




impl Runner  {
    pub async fn api_refresh_loop(&self, sec: u64) {
        let cache_api = Arc::clone(&self.cache);
        let chain_id = self.config.chain_id;
         tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(sec)).await;
                cache_api.api_refresh(chain_id).await;
                for id in cache_api.ids() {
                   let _ =  cache_api.recompute_all_hf(id);
                   let _ =  cache_api.sort_by_hf(id);  
                }
            }
        });
    }

   
    
}
