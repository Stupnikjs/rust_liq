use crate::{cache::MarketCache};
use connector::Connector;
use eth_core::traits::RpcKind; 
use crate::runner::{Runner}; 
use crate::config::Config; 
use crate::swap::{routes::RouteCache, quoter::UniswapV3}; 
use std::fs;
use tokio::sync::RwLock; 
use std::sync::{Arc};
use std::str::FromStr; 



struct QuoteConsumer {
    route_cache: Arc<RwLock<RouteCache>>,
    cache: Arc<MarketCache>,
    connector: Arc<Connector>, 
    config: Arc<Config>,
}



impl QuoteConsumer {

    pub async fn quote_market(&self) -> Result<(), Box<dyn std::error::Error>> {
        let _route_cache = Arc::clone(&self.route_cache);
        for id in self.cache.ids() {
            let _ = self.cache.onchain_oracle_refresh(&self.connector, RpcKind::Secondary, id).await; 
            let param = self.cache.get_market_param_by_id(id).expect("error in runner init get market param"); 
            let swaper = UniswapV3::new(
                self.config.dexes[0].quoter, 
                self.config.dexes[0].router, 
                1000, 
                String::from_str("UNIV3")?); 

            let snap = self.cache.snapshot(id).expect("snap failed in quote init"); 
            let edge = swaper.best_amount_in(
                &self.connector, 
                param.collateral_token, 
                param.loan_token, 
                snap.stats.max_collateral_pos, 
                snap.stats.oracle_price, 
                param.max_slippage()).await; 
            
            let Some(edge) = edge else {
            self.cache.update(id, |m| m.canceled = true);
            continue;
            };
            let mut route_cache = self.route_cache.write().await; 
            route_cache.insert_edge(id, edge);
            println!("found swap for {}", snap.params.get_pair()); 
        }

         Ok(())
    }



}


impl Runner {
    pub async fn quote_market(&self)  {
        let qc = QuoteConsumer{
                cache: Arc::clone(&self.cache),
                connector: Arc::clone(&self.connector),
                route_cache: Arc::clone(&self.route_cache),
                config:Arc::clone(&self.config), 
        }; 
        let _ =  qc.quote_market().await; 
    }
}