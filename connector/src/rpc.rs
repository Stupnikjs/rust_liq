use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH}; 
use alloy::network::Ethereum;
use alloy::providers::{Provider, ProviderBuilder, RootProvider};
use alloy::rpc::client::WsConnect;
use alloy::rpc::types::{BlockNumberOrTag, Filter, Log, TransactionRequest};
use alloy::signers::local::PrivateKeySigner;
use alloy::primitives::{Address, Bytes, TxHash, address};
use eth_core::traits::{CallRaw}; 
use crate::bucket::Bucket; 
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
    next_ok_at: AtomicU64,
    pub consecutive_failures:AtomicU64, 
    buckets: [Bucket; NUM_BUCKETS],
    latency_ema:AtomicU64,  
}


#[derive(Debug, serde::Serialize)]
pub struct RpcInfo {
    pub url: String,
    pub tier: String,
    pub failures: u64,
    pub success_rate_60m: f64,
    pub  cooldown_ms:u64,
}



pub struct  CallStats {
    pub latency_ms: u64,
    pub call_type: CallType, 
}

#[derive(Debug, Clone)]
pub enum CallType {
    MarketCall,
    OracleCall,
    LiquidationCall,
}


/*


┌─────────────────────────────────────────────────────────────┐
│                        RpcPool                              │
└─────────────────────────────────────────────────────────────┘

• Gère un ensemble d'endpoints RPC.

• acquire() et acquire_top_tier() acquire_lowest_latency_top_tier()
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


top_tier_low_latency => liquidation  => tier0
low_latency => oracle proche liquidation tier1
public => oracle/market loin de liquidation tier2 


*/

static START: OnceLock<Instant> = OnceLock::new();
const BUCKET_SECS: u64 = 60;          // 1 bucket par minute
const NUM_BUCKETS: usize = 60;         // 60 buckets = 1h de fenêtre max

impl RpcEndpoint {
        pub fn new(url: String, tier: Tier, min_interval: Duration) -> anyhow::Result<Self> {
        
            Ok(Self {
                provider: RootProvider::<Ethereum>::new_http(url.parse()?),
                url,
                tier,
                min_interval,
                next_ok_at: AtomicU64::new(0),
                consecutive_failures: AtomicU64::new(0),
                buckets: std::array::from_fn(|_| Bucket::new()),
                latency_ema: AtomicU64::new(u64::MAX),
            })
        }

    
        fn try_reserve(&self) -> bool {
            loop{
                let now = current_millis();
                let next = self.next_ok_at.load(Ordering::Acquire);
                if now < next {
                    return false;
                }
                let new_next = now + self.min_interval.as_millis() as u64;

                match self.next_ok_at.compare_exchange_weak(
                next,
                new_next,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return true,
                Err(_) => continue,
            }
        }
    }
        pub fn register_success(&self) {
            self.consecutive_failures.store(0, Ordering::Relaxed);
            let epoch = current_epoch();
            self.buckets[bucket_index(epoch)].record(epoch, true);
            
        }

        pub fn register_failure(&self) {
        let failures = self.consecutive_failures.fetch_add(1, Ordering::Relaxed) + 1;

        // 2^failures max 60sec 
        let secs = (1u64 << failures.min(5)).min(60);

        
        let new_next = current_millis() + secs * 1000;
        let slot = self.next_ok_at.store(new_next, Ordering::Relaxed);
        let epoch = current_epoch();
        self.buckets[bucket_index(epoch)].record(epoch, false);
       }

        pub fn failures(&self) -> u64 {
            self.consecutive_failures.load(Ordering::Relaxed)
    }


    /// Taux de succès sur les `window_minutes` dernières minutes (max NUM_BUCKETS).
    pub fn success_rate(&self, window_minutes: u64) -> f64 {
        let now_epoch = current_epoch();
        let n = window_minutes.min(NUM_BUCKETS as u64);

        let mut total_attempts = 0u64;
        let mut total_successes = 0u64;

        for i in 0..n {
            let epoch = now_epoch.saturating_sub(i);
            let (attempts, successes) = self.buckets[bucket_index(epoch)].read(epoch);
            total_attempts += attempts;
            total_successes += successes;
        }

        if total_attempts == 0 {
            return 1.0; // pas de data récente, convention à ajuster selon ton usage
        }
        total_successes as f64 / total_attempts as f64
    }

        pub fn cooldown_ms(&self) -> u64 {
        let now = current_millis();
        let next = self.next_ok_at.load(Ordering::Acquire);

        next.saturating_sub(now)
    }


    pub fn record_latency(&self, latency_ms: u64) {
    loop {
        let current = self.latency_ema.load(Ordering::Acquire);
        let new_val = if current == u64::MAX {
            latency_ms
        } else {
            let diff = latency_ms as i64 - current as i64;
            (current as i64 + diff / 8) as u64
        };
        if self.latency_ema
            .compare_exchange_weak(current, new_val, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            return;
        }
        }
    }
    /// None si jamais mesuré.
    pub fn latency_ms(&self) -> Option<u64> {
        let v = self.latency_ema.load(Ordering::Acquire);
        (v != u64::MAX).then_some(v)
    }

    
    
}

pub struct RpcPool {
    endpoints: Vec<Arc<RpcEndpoint>>,
    rr_counter: AtomicU64,
}

impl  RpcPool {
    pub fn new(endpoints: Vec<Arc<RpcEndpoint>>) -> Self {
        let rr_counter = AtomicU64::new(0); 
        Self { endpoints,  rr_counter}
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


pub async fn acquire_lowest_latency_top_tier(&self) -> anyhow::Result<&Arc<RpcEndpoint>> {
    let mut top: Vec<&Arc<RpcEndpoint>> = self.endpoints.iter()
        .filter(|e| e.tier == Tier::Top)
        .collect();

    if top.is_empty() {
        return Err(anyhow::anyhow!("no top tier endpoints configured"));
    }

    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            // endpoints jamais mesurés (None) en dernier : priorité aux latences connues
            top.sort_by_key(|e| e.latency_ms().unwrap_or(u64::MAX));

            if let Some(ep) = top.iter().find(|e| e.try_reserve()) {
                return *ep;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .map_err(|_| anyhow::anyhow!("no top tier rpc available"))
}

// pour eth_call sur hf < 1.005 
pub async fn acquire_lowest_score(&self) -> anyhow::Result<&Arc<RpcEndpoint>> {
    let mut candidates: Vec<&Arc<RpcEndpoint>> = self.endpoints.iter().collect();

    if candidates.is_empty() {
        return Err(anyhow::anyhow!("no endpoints configured"));
    }

    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            candidates.sort_by(|a, b| {
                let score_a = endpoint_score(a.latency_ms(), a.success_rate(60));
                let score_b = endpoint_score(b.latency_ms(), b.success_rate(60));
                score_a.partial_cmp(&score_b).unwrap_or(std::cmp::Ordering::Equal)
            });

            if let Some(ep) = candidates.iter().find(|e| e.try_reserve()) {
                return *ep;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .map_err(|_| anyhow::anyhow!("no rpc available"))
}

    
    pub fn info(&self) -> Vec<RpcInfo> {
    self.endpoints
        .iter()
        .map(|ep| RpcInfo {
            url: ep.url.clone(),
            tier: match ep.tier {
                Tier::Top => "Top",
                Tier::Garbage => "Garbage",
            }
            .to_string(),
            failures: ep.failures(),
            success_rate_60m: ep.success_rate(60),
            cooldown_ms: ep.cooldown_ms(),
        })
        .collect()
}

}


fn current_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() / BUCKET_SECS
}

fn bucket_index(epoch: u64) -> usize {
    (epoch % NUM_BUCKETS as u64) as usize
}

fn current_millis() -> u64 {
    START
        .get_or_init(Instant::now)
        .elapsed()
        .as_millis() as u64
}


fn endpoint_score(latency_ms: Option<u64>, success_rate: f64) -> f64 {
    const UNMEASURED_PENALTY_MS: f64 = 2000.0;
    const MIN_SUCCESS_RATE: f64 = 0.05;

    let latency = latency_ms.map(|l| l as f64).unwrap_or(UNMEASURED_PENALTY_MS);
    let success_rate = success_rate.max(MIN_SUCCESS_RATE);
    latency / success_rate
}





