#![allow(dead_code, unused_variables, unused_imports)]
use std::sync::Arc;
use alloy::network::Ethereum;
use alloy::providers::{Provider, ProviderBuilder, RootProvider};
use alloy::rpc::client::WsConnect;
use alloy::rpc::types::{BlockNumberOrTag, Filter, Log, TransactionRequest};
use alloy::signers::local::PrivateKeySigner;
use alloy::primitives::{Address, Bytes, TxHash, address};
use eth_core::traits::{CallRaw }; 
use eth_core::utils::BoxError;
use futures::StreamExt;
use tx_sender::TxSender;
use rpc::{RpcPool, RpcEndpoint};
use tokio::sync::Semaphore;
use tokio::time::{interval, Duration};

mod tx_sender;
pub mod rpc;


/*


Architecture :

- RpcPool gère les endpoints RPC.
  - sélection des endpoints disponibles ;
  - suivi des succès/échecs ;
  - backoff exponentiel après les failures (jusqu'à 60 s).

- TxSender gère :
  - la synchronisation et l'incrémentation du nonce ;
  - l'envoi des transactions (toujours via un endpoint top-tier).

- Connector :
  - fournit les appels eth_call en choisissant un endpoint adapté ;
  - gère la souscription WebSocket aux événements ;
  - délègue l'envoi des transactions à TxSender.

- Les Arc permettent le partage des ressources entre les tâches Tokio.



*/

pub struct Connector {
    pub pool: RpcPool,
    pub ws: Arc<RootProvider<Ethereum>>,
    pub tx_sender: Arc<TxSender>,
}

impl Connector {
 
    pub async fn subscribe<F>(&self, morpho_addr: Address, events_sig: &[&str],  mut on_log: F) -> Result<(), BoxError>
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

    pub async fn send_tx(&self, to: Address, data: Bytes) -> Result<TxHash, BoxError> {
        self.tx_sender.send_tx(&self.pool.acquire_top_tier().await.unwrap().provider, to, data).await
    }
}

pub async fn build(
    rpc_configs: Vec<Arc<RpcEndpoint>>,
    ws_url: &str,
    signer: PrivateKeySigner,
    chain_id: u64,
) -> Result<Connector, BoxError> {
    let ws = Arc::new(
        ProviderBuilder::new()
            .disable_recommended_fillers()
            .connect_ws(WsConnect::new(ws_url))
            .await?,
    );

    let pool = RpcPool::new(rpc_configs);

    // le tx_sender s'appuie sur un endpoint top-tier dès l'init
    let init_ep = pool.acquire_top_tier().await?;

    let tx_sender = Arc::new(TxSender::init(&init_ep.provider, signer, chain_id).await?);
    tx_sender.spawn_base_fee_updater(Arc::clone(&ws));

    Ok(Connector { pool, ws, tx_sender })
}







#[async_trait::async_trait]
impl CallRaw for Connector {
    async fn call_raw(
        &self,
        tier:u8, 
        from: Address,
        to: Address,
        data: Bytes,
    ) -> Result<Bytes, BoxError> {
        const MAX_RETRIES: u32 = 3;
    let tx = TransactionRequest::default().from(from).to(to).input(data.into());


    for attempt in 0..MAX_RETRIES {
        let ep = if tier == 0 {
            match self.pool.acquire_top_tier().await {
                Ok(ep) => {
                    ep
                },
                Err(_) => {
 
                    match self.pool.acquire().await {
                    Ok(ep) => ep,
                    Err(_) => {
                        tokio::time::sleep(Duration::from_secs(2)).await;
                        continue;
                    }
                }
                },
            }
        } else {
            match self.pool.acquire().await {
                Ok(ep) => ep,
                Err(_) => {
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    continue;
                }
            }
        };

        
        match ep.provider.call(tx.clone()).await {
            Ok(bytes) => {
                ep.register_success();
                return Ok(bytes);
            }
            Err(err) => {
                ep.register_failure();
            }
        }
    }

        Err(BoxError::from("max retry reached "))
}
}


