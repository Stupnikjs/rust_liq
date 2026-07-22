#![allow(dead_code, unused_variables, unused_imports)]
use std::sync::Arc;
use alloy::network::Ethereum;
use alloy::providers::{Provider, ProviderBuilder, RootProvider};
use alloy::rpc::client::WsConnect;
use alloy::rpc::types::{BlockNumberOrTag, Filter, Log, TransactionRequest};
use alloy::signers::local::PrivateKeySigner;
use alloy::primitives::{Address, Bytes, TxHash, address};
use eth_core::traits::{CallRaw, RpcKind}; 
use futures::StreamExt;
use tx_sender::TxSender;
use rpc::{RpcPool, RpcEndpoint};
use tokio::sync::Semaphore;
use tokio::time::{interval, Duration};


mod tx_sender;
pub mod rpc;


pub struct Connector {
    pub pool: RpcPool,
    pub ws: Arc<RootProvider<Ethereum>>,
    pub tx_sender: Arc<TxSender>,
}
// address!("78D3FEc647f35E5D413597D217C5E0D9605acE3E")
impl Connector {
    pub async fn call_raw(&self, top_tier:bool, from: Address, to: Address, data: Bytes) -> Result<Bytes, Box<dyn std::error::Error>> {
        let ep = if top_tier { self.pool.acquire_top_tier().await } else {self.pool.acquire().await}; 
        let tx = TransactionRequest::default().from(from).to(to).input(data.into());
        match ep.provider.call(tx).await {
        Ok(bytes) => {
            ep.register_success();
            Ok(bytes)
        }
        Err(err) => {
            ep.register_failure();
            Err(err.into())
        }
    }
    }
    

    

    pub async fn subscribe<F>(&self, morpho_addr: Address, events_sig: &[&str],  mut on_log: F) -> Result<(), Box<dyn std::error::Error>>
    where
        F: FnMut(Log),
    {
        let filter = Filter::new()
            .address(morpho_addr)
            .from_block(BlockNumberOrTag::Latest)
            .events(events_sig);

        let sub = self.ws.subscribe_logs(&filter).await?;
        let mut stream = sub.into_stream();
        while let Some(log) = stream.next().await {
            if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                on_log(log);
            }))
            .is_err()
            {
                return Err("callback panicked".into())
        }
      }
        Err("ws log subscription stream ended".into())
    }

    pub async fn send_tx(&self, to: Address, data: Bytes) -> Result<TxHash, Box<dyn std::error::Error>> {
        self.tx_sender.send_tx(&self.pool.acquire_top_tier().await.provider, to, data).await
    }
}

pub async fn build(
    rpc_configs: Vec<Arc<RpcEndpoint>>,
    ws_url: &str,
    signer: PrivateKeySigner,
    chain_id: u64,
) -> Result<Connector, Box<dyn std::error::Error>> {
    let ws = Arc::new(
        ProviderBuilder::new()
            .disable_recommended_fillers()
            .connect_ws(WsConnect::new(ws_url))
            .await?,
    );

    let pool = RpcPool::new(rpc_configs);

    // le tx_sender s'appuie sur un endpoint top-tier dès l'init
    let init_ep = pool.acquire_top_tier().await;
    let tx_sender = Arc::new(TxSender::init(&init_ep.provider, signer, chain_id).await?);
    tx_sender.spawn_base_fee_updater(Arc::clone(&ws));

    Ok(Connector { pool, ws, tx_sender })
}







#[async_trait::async_trait]
impl CallRaw for Connector {
    async fn call_raw(
        &self,
        top_tier:bool, 
        from: Address,
        to: Address,
        data: Bytes,
    ) -> Result<Bytes, Box<dyn std::error::Error>> {
        let ep = if top_tier {self.pool.acquire_top_tier().await} else {self.pool.acquire().await }; 
        let tx = TransactionRequest::default().from(from).to(to).input(data.into());
        match ep.provider.call(tx).await {
        Ok(bytes) => {
            ep.register_success();
            Ok(bytes)
        }
        Err(err) => {
            ep.register_failure();
            Err(err.into())
        }
    }
}
}



// keep track of err in RPC 