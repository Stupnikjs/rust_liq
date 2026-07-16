#![allow(dead_code, unused_variables, unused_imports)]
use std::sync::Arc;
use alloy::network::Ethereum;
use alloy::providers::{Provider, ProviderBuilder, RootProvider};
use alloy::rpc::client::WsConnect;
use alloy::rpc::types::{BlockNumberOrTag, Filter, Log, TransactionRequest};
use alloy::signers::local::PrivateKeySigner;
use alloy::primitives::{Address, Bytes, TxHash, address};
use futures::StreamExt;
use tx_sender::TxSender;
use tokio::sync::Semaphore;
use tokio::time::{interval, Duration};


mod tx_sender;




pub struct Connector {
    pub http: RootProvider<Ethereum>,
    pub sec_http: RootProvider<Ethereum>,
    pub ws: Arc<RootProvider<Ethereum>>,
    pub tx_sender: Arc<TxSender>,
    pub rate_limiter: RateLimiter,
}

impl Connector {
    pub async fn call_raw(&self, to: Address, data: Bytes) -> Result<Bytes, Box<dyn std::error::Error>> {
        self.rate_limiter.acquire().await;
        let tx = TransactionRequest::default()
        .from(address!("78D3FEc647f35E5D413597D217C5E0D9605acE3E"))  // change asap 
        .to(to)
        .input(data.into());
        Ok(self.http.call(tx).await?)
    }

    pub async fn sec_call_raw(&self, to: Address, data: Bytes) -> Result<Bytes, Box<dyn std::error::Error>> {
        self.rate_limiter.acquire().await;
        let tx = TransactionRequest::default().to(to).input(data.into());
        Ok(self.sec_http.call(tx).await?)
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
            on_log(log);
        }
        Err("ws log subscription stream ended".into())
    }

    pub async fn send_tx(&self, to: Address, data: Bytes) -> Result<TxHash, Box<dyn std::error::Error>> {
        self.tx_sender.send_tx(&self.http, to, data).await
    }
}

pub async fn build(
    http_url: &str,
    sec_http_url: &str,
    ws_url: &str,
    signer: PrivateKeySigner,
    chain_id: u64,
    max_rps: usize,
) -> Result<Connector, Box<dyn std::error::Error>> {
    let http = RootProvider::<Ethereum>::new_http(http_url.parse()?);
    let sec_http = RootProvider::<Ethereum>::new_http(sec_http_url.parse()?);
    let ws = Arc::new(
        ProviderBuilder::new()
            .disable_recommended_fillers()
            .connect_ws(WsConnect::new(ws_url))
            .await?,
    );

    let rate_limiter = RateLimiter::new(max_rps);
    let tx_sender = Arc::new(TxSender::init(&http, signer, chain_id).await?);
    tx_sender.spawn_base_fee_updater(Arc::clone(&ws));

    Ok(Connector { http,sec_http, ws, tx_sender, rate_limiter })
}




#[derive(Clone)]
pub struct RateLimiter {
    semaphore: Arc<Semaphore>,
    max_tokens: usize,
}

impl RateLimiter {
    /// `max_rps` = requêtes max par seconde
    pub fn new(max_rps: usize) -> Self {
        let semaphore = Arc::new(Semaphore::new(max_rps));
        let sem_clone = semaphore.clone();

        tokio::spawn(async move {
            let mut ticker = interval(Duration::from_secs(1));
            loop {
                ticker.tick().await;
                let current = sem_clone.available_permits();
                let to_add = max_rps.saturating_sub(current);
                sem_clone.add_permits(to_add);
            }
        });

        Self {
            semaphore,
            max_tokens: max_rps,
        }
    }

    pub async fn acquire(&self) {
        self.semaphore
            .acquire()
            .await
            .expect("semaphore closed")
            .forget(); // consume le permit sans le rendre
    }
}



#[async_trait::async_trait]
pub trait CallRaw {
    async fn call_raw(
        &self,
        to: Address,
        data: Bytes,
    ) -> Result<Bytes, Box<dyn std::error::Error>>;
}



#[async_trait::async_trait]
impl CallRaw for Connector {
    async fn call_raw(
        &self,
        to: Address,
        data: Bytes,
    ) -> Result<Bytes, Box<dyn std::error::Error>> {
    self.rate_limiter.acquire().await;
        let tx = TransactionRequest::default()
        .from(address!("78D3FEc647f35E5D413597D217C5E0D9605acE3E"))  // change asap 
        .to(to)
        .input(data.into());
        Ok(self.http.call(tx).await?)
    }
}