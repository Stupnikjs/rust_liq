# Améliorations proposées — `market_loop.rs`

## ⚠️ Bug de compilation immédiat

```rust
use std::sync::{Arc};              // RwLock manquant dans l'import
route_cache: Arc<RwLock<RouteCache>>,  // ne compile pas tel quel
```

Migration `std::sync::RwLock` → `tokio::sync::RwLock` en cours (cf. commentaire dans le
code). Attention à l'API différente : `tokio::sync::RwLock::read()` est `async` et retourne
directement le guard, pas un `Result` :

```rust
// avant (std, avec risque de poisoning — bug #6 de bugs.md)
let route = self.route_cache.read().unwrap().get_edge(&pos.market_id).cloned();

// après (tokio, pas de poisoning possible)
let route = self.route_cache.read().await.get_edge(&pos.market_id).cloned();
```

---

## Efficacité des rounds (réduire le nombre d'appels RPC)

| # | Problème | Solution |
|---|----------|----------|
| 1 | `refresh_oracle_and_hf` appelle l'oracle à chaque tick, même si plusieurs marchés partagent le même oracle | Cache d'oracle partagé, keyé par adresse d'oracle (pas par marché), avec `max_age` |
| 2 | Un `eth_call` séparé par marché par tick | Batcher via **Multicall3** (contrat standard déployé partout) : N calls → 1 |
| 3 | Tous les `MarketLoopConsumer` démarrent `count = 0` en même temps → `market_refresh_and_sort` (le `% 10`) se déclenche pour tous les marchés au même tick (effet de troupeau) | Décaler le modulo par un offset dérivé de l'`id` du marché |
| 4 | `interval == 0` en continu si une liquidation échoue à répétition → spin-loop (bug #7) | Backoff exponentiel basé sur `spaming_map.attempts`, plafonné (`.max(backoff)`) |

### Cache d'oracle partagé — squelette
```rust
struct OracleCache {
    prices: RwLock<HashMap<Address, (U256, Instant)>>,
}
impl OracleCache {
    async fn get_or_refresh(&self, oracle: Address, connector: &Connector, max_age: Duration) -> U256 {
        if let Some((price, t)) = self.prices.read().await.get(&oracle) {
            if t.elapsed() < max_age { return *price; }
        }
        let price = connector.call_oracle(oracle).await;
        self.prices.write().await.insert(oracle, (price, Instant::now()));
        price
    }
}
```

### Offset anti-effet-de-troupeau
```rust
let offset = (self.id.as_slice()[0] as u64) % 10;
if (count + offset) % 10 == 0 {
    self.market_refresh_and_sort().await;
}
```

### Backoff progressif
```rust
let attempts = self.spaming_map.get(&pos.address).map(|a| a.count).unwrap_or(0);
let backoff = (2u64.pow(attempts.min(6) as u32)).min(60); // 1,2,4...60s max
let sleep_secs = interval.max(backoff);
tokio::time::sleep(Duration::from_secs(sleep_secs)).await;
```

---

## Scaling (architecture)

**Design actuel :** un `tokio::spawn` indépendant par marché, chacun avec son propre
`sleep(interval)`. Simple, mais deux limites au-delà de quelques dizaines de marchés :

- Pas de vue globale sur le débit RPC total — le `rate_limiter` du connector met tout le
  monde en file d'attente **sans priorité** (un marché critique peut attendre derrière un
  marché sans urgence).
- Pas de limite sur le nombre de refresh **concurrents** — si beaucoup de marchés se
  réveillent la même seconde, toutes les requêtes partent avant que le rate limiter ne les
  étale.

**Alternative proposée : scheduler central à file de priorité + sémaphore**, au lieu de N
boucles indépendantes.

```rust
struct Scheduler {
    queue: BinaryHeap<Reverse<(Instant, FixedBytes<32>)>>, // min-heap par prochain wake
    semaphore: Arc<Semaphore>, // borne le nb de refresh RPC concurrents
}

async fn run_scheduler(mut sched: Scheduler, cache: Arc<MarketCache>) {
    loop {
        let Some(Reverse((wake_at, market_id))) = sched.queue.peek().cloned() else {
            tokio::time::sleep(Duration::from_secs(1)).await;
            continue;
        };
        tokio::time::sleep_until(wake_at.into()).await;
        sched.queue.pop();

        let permit = sched.semaphore.clone().acquire_owned().await.unwrap();
        let cache = cache.clone();
        tokio::spawn(async move {
            let interval = process_one_market(market_id, &cache).await;
            drop(permit);
            // repousser dans la queue avec le nouvel interval calculé
        });
    }
}
```

**Bénéfices :**
- Priorité naturelle : les marchés à HF critique (interval court) remontent en haut du tas,
  traités en premier même sous forte charge.
- `Semaphore` borne le nombre de refresh RPC simultanés quel que soit le nombre total de
  marchés — pression sur le rate limit contrôlée directement, pas subie.
- Point unique pour instrumenter (temps d'attente moyen en queue, RPC/s, taux d'échec).
- Supervision centralisée : un panic dans `process_one_market` ne tue pas le scheduler
  lui-même — corrige au passage le manque de tracking de `JoinHandle` (bug #1 de `bugs.md`).

**Ce qui reste bon dans le design actuel** : le principe d'un `interval` calculé
dynamiquement par marché (`lowest_hf_and_interval`) est une bonne base de priorisation — le
changement structurel proposé est de sortir cette logique de N boucles indépendantes vers une
priorisation **globale** via une seule file.
               |