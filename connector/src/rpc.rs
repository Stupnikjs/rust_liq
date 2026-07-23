use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, Arc};
use std::time::{Instant}; 
use alloy::network::Ethereum;
use alloy::providers::{Provider, ProviderBuilder, RootProvider};
use alloy::rpc::client::WsConnect;
use alloy::rpc::types::{BlockNumberOrTag, Filter, Log, TransactionRequest};
use alloy::signers::local::PrivateKeySigner;
use alloy::primitives::{Address, Bytes, TxHash, address};
use eth_core::traits::{CallRaw}; 
use futures::StreamExt;
use tokio::sync::{Semaphore};
use tokio::time::{interval, Duration};


const MAX_FAILURES:u64 = 5; 

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    Top,
    Garbage,
}

pub struct RpcEndpoint {
    pub url: String,
    pub tier: Tier,
    pub provider: RootProvider<Ethereum>,
    pub min_interval: Duration,
    next_ok_at: Mutex<Instant>,
    pub consecutive_failures:AtomicU64, 
}


/*


┌─────────────────────────────────────────────────────────────┐
│                        RpcPool                              │
└─────────────────────────────────────────────────────────────┘

• Gère un ensemble d'endpoints RPC.

• acquire() et acquire_top_tier()
  → sélectionnent un endpoint disponible
  → attendent au maximum 2 secondes avant d'abandonner.

• try_reserve()
  → réserve atomiquement un endpoint disponible ;
  → retourne true si la réservation a réussi.

• Gestion des échecs
  → chaque failure augmente un backoff exponentiel
    (jusqu'à 60 s) avant que l'endpoint puisse être réutilisé.

L'objectif est de répartir les appels RPC, d'éviter les endpoints
défaillants et de limiter les requêtes simultanées vers un même nœud.


*/


impl RpcEndpoint {
        pub fn new(url: String, tier: Tier, min_interval: Duration) -> anyhow::Result<Self> {
            Ok(Self {
                provider: RootProvider::<Ethereum>::new_http(url.parse()?),
                url,
                tier,
                min_interval,
                next_ok_at: Mutex::new(Instant::now()),
                consecutive_failures: AtomicU64::new(0),
            })
        }

    /// true si le cooldown est passé — et le réserve immédiatement pour éviter une race.
        fn try_reserve(&self) -> bool {
            let mut slot = self.next_ok_at.lock().unwrap();
            let now = Instant::now();
            if now >= *slot {
                *slot = now + self.min_interval;
                true
            } else {
                false
            }
        }

        pub fn register_success(&self) {
            self.consecutive_failures.store(0, Ordering::Relaxed);
        }

        pub fn register_failure(&self) {
        let failures = self.consecutive_failures.fetch_add(1, Ordering::Relaxed) + 1;

        // 2^failures max 60sec 
        let secs = (1u64 << failures.min(5)).min(60);

        let mut slot = self.next_ok_at.lock().unwrap();
        *slot = Instant::now() + Duration::from_secs(secs);
    }

        pub fn failures(&self) -> u64 {
            self.consecutive_failures.load(Ordering::Relaxed)
    }
   
}

pub struct RpcPool {
    endpoints: Vec<Arc<RpcEndpoint>>,
}

impl  RpcPool {
    pub fn new(endpoints: Vec<Arc<RpcEndpoint>>) -> Self {
        Self { endpoints }
    }
    

    pub async fn acquire(&self) -> anyhow::Result<&Arc<RpcEndpoint>> {
    tokio::time::timeout(
        Duration::from_secs(2),
        async {
            loop {
                if let Some(ep) = self.endpoints
                    .iter()
                    .find(|e| e.try_reserve())
                {

                    return ep;
                }

                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        }
    )
    .await
    .map_err(|_| anyhow::anyhow!("no rpc available"))
    }


    pub async fn acquire_top_tier(&self) -> anyhow::Result<&Arc<RpcEndpoint>> {
        tokio::time::timeout(
        Duration::from_secs(2),
        async {
        loop {
            if let Some(ep) = self.endpoints.iter().filter(|e| e.tier == Tier::Top).find(|e| e.try_reserve()) {
                return ep;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }).await
    .map_err(|_| anyhow::anyhow!("no top tier rpc available"))
    }
}