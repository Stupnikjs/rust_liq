#![allow(dead_code, unused_variables, unused_imports)]
use std::sync::Arc;
use std::sync::RwLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use alloy::consensus::{SignableTransaction, TxEip1559};
use alloy::network::TxSignerSync;
use alloy::primitives::{Address, Bytes, TxHash, U256};
use alloy::providers::Provider;
use alloy::rpc::types::{BlockNumberOrTag, TransactionRequest};
use alloy::signers::local::PrivateKeySigner;
use eth_core::utils::BoxError;
use futures::StreamExt;
use std::collections::BTreeSet;
use std::sync::Mutex;


pub struct TxSender {
    signer: PrivateKeySigner,
    next_nonce: AtomicU64,
    released_nonces: Mutex<BTreeSet<u64>>,
    base_fee: RwLock<u128>,
    chain_id: u64,
}

impl TxSender {
    pub async fn init<P: Provider>(
        http: &P,
        signer: PrivateKeySigner,
        chain_id: u64,
    ) -> Result<Self, BoxError> {
        let nonce = http
            .get_transaction_count(signer.address())
            .pending()
            .await?;

        Ok(Self {
            signer,
            next_nonce: AtomicU64::new(nonce),
            released_nonces: Mutex::new(BTreeSet::new()),
            base_fee: RwLock::new(0),
            chain_id,
        })
    }

    pub fn address(&self) -> Address {
        self.signer.address()
    }

    fn base_fee(&self) -> u128 {
        *self.base_fee.read().unwrap()
    }

    fn set_base_fee(&self, v: u128) {
        *self.base_fee.write().unwrap() = v;
    }

    /// Spawns a background task keeping base_fee up to date via block subscription.
    pub fn spawn_base_fee_updater<P>(self: &Arc<Self>, ws: Arc<P>)
    where
        P: Provider + Send + Sync + 'static,
    {
        let sender = Arc::clone(self);
        tokio::spawn(async move {
            loop {
                match ws.subscribe_blocks().await {
                    Ok(sub) => {
                        let mut stream = sub.into_stream();
                        while let Some(header) = stream.next().await {
                            if let Some(fee) = header.base_fee_per_gas {
                                sender.set_base_fee(fee as u128);
                            }
                        }
                        eprintln!("base fee subscription ended, reconnecting");
                    }
                    Err(e) => {
                        eprintln!("subscribe_blocks failed: {e}, retrying in 2s");
                        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    }
                }
            }
        });
    }

    pub async fn send_tx<P: Provider>(
        &self,
        http: &P,
        to: Address,
        data: Bytes,
    ) -> Result<TxHash, BoxError> {
        
        let from = self.address();
        let nonce = self.next_nonce();
        let guard = NonceGuard::new(self, nonce);


        let mut base_fee = self.base_fee();
        if base_fee == 0 {
            // fallback if the updater hasn't ticked yet (e.g. right at startup)
            base_fee = http
                .get_block_by_number(BlockNumberOrTag::Latest)
                .await?
                .ok_or("no latest block")?
                .header
                .base_fee_per_gas
                .ok_or("no base fee")? as u128;
        }


        let max_priority_fee = 1_000_000u128;
        let max_fee = base_fee + max_priority_fee;

        let tx_req = TransactionRequest::default()
            .from(from)
            .to(to)
            .input(data.clone().into());

        let gas_limit = http.estimate_gas(tx_req).await?;

        let mut tx = TxEip1559 {
            chain_id: self.chain_id,
            nonce,
            max_fee_per_gas: max_fee,
            max_priority_fee_per_gas: max_priority_fee,
            gas_limit,
            to: alloy::primitives::TxKind::Call(to),
            value: U256::ZERO,
            input: data,
            access_list: Default::default(),
        };

        let sig = self.signer.sign_transaction_sync(&mut tx)?;
        let signed = tx.into_signed(sig);

        let mut buf = vec![];
        signed.eip2718_encode(&mut buf);
        let pending = http.send_raw_transaction(&buf).await?;
         let tx_hash = *signed.hash();
        guard.disarm();

        let deadline = tokio::time::Instant::now() + Duration::from_secs(30);

        let receipt = loop {
            if let Some(r) = http.get_transaction_receipt(tx_hash).await? {
                break Some(r);
            }
            if tokio::time::Instant::now() >= deadline {
                break None;
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        };

        match receipt {
            Some(r) => Ok(r.transaction_hash),
            None => {
                // 30s écoulées sans receipt : on distingue "toujours en vol" de "droppée"
                match check_tx_status(http, tx_hash).await {
                    TxStatus::Mined => {
                        // cas rare : minée juste après notre dernier poll
                        let r = http
                            .get_transaction_receipt(tx_hash)
                            .await?
                            .ok_or("tx reported mined but receipt not found")?;
                        Ok(r.transaction_hash)
                    }
                    TxStatus::StillPending => {
                        Err(format!(
                            "tx {:?} still pending in mempool after 30s, nonce {} remains reserved",
                            tx_hash, nonce
                        ).into())
                    }
                    TxStatus::Dropped => {
                        self.release_nonce(nonce);
                        Err(format!(
                            "tx {:?} dropped from mempool, nonce {} released for reuse",
                            tx_hash, nonce
                        ).into())
                    }
                    TxStatus::Unknown => {
                        Err(format!(
                            "tx {:?} status unknown after RPC error, nonce {} kept reserved to be safe",
                            tx_hash, nonce
                        ).into())
                    }
                }
            }
        }
    }
    // priorité : réutiliser un gap si disponible
    fn next_nonce(&self) -> u64 {
        
        // lock activé 
        let mut released = self.released_nonces.lock().unwrap();
        
        // recupere plus petit nonce et retourne si gap 
        if let Some(&n) = released.iter().next() {
            // remove du Btree
            released.remove(&n);
            return n;
        }
        // libere le lock du mutex 
        drop(released);
        
        // pas de gap, incremente le nonce 
        self.next_nonce.fetch_add(1, Ordering::SeqCst)
    }

    fn release_nonce(&self, nonce: u64) {
        // toujours safe : on remet le nonce dans le pool, peu importe l'ordre
        self.released_nonces.lock().unwrap().insert(nonce);
    }
}






struct NonceGuard<'a> {
    sender: &'a TxSender,
    nonce: u64,
    armed: bool,
}

impl<'a> NonceGuard<'a> {
    fn new(sender: &'a TxSender, nonce: u64) -> Self {
        Self { sender, nonce, armed: true }
    }

    /// À appeler une fois qu'on est certain que le nonce a été consommé
    /// on-chain (tx broadcastée avec succès) — plus jamais à réutiliser.
    fn disarm(mut self) {
        self.armed = false;
    }
}

impl<'a> Drop for NonceGuard<'a> {
    fn drop(&mut self) {
        if self.armed {
            self.sender.release_nonce(self.nonce);
        }
    }
}



async fn check_tx_status<P: Provider>(
    http: &P,
    tx_hash: TxHash,
) -> TxStatus {
    match http.get_transaction_by_hash(tx_hash).await {
        Ok(Some(tx)) => {
            if tx.block_number.is_some() {
                // minée entre-temps, juste pas encore indexée côté receipt
                TxStatus::Mined
            } else {
                // toujours dans le mempool, peut encore atterrir
                TxStatus::StillPending
            }
        }
        Ok(None) => {
            // plus connue du node : droppée du mempool (remplacée, expirée, etc.)
            TxStatus::Dropped
        }
        Err(e) => {
            eprintln!("check_tx_status: rpc error checking {tx_hash}: {e}");
            // on ne sait pas -> on ne prend pas le risque de relâcher le nonce
            TxStatus::Unknown
        }
    }
}

enum TxStatus {
    Mined,
    StillPending,
    Dropped,
    Unknown,
} 