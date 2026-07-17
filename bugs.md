# Review `rust_liq` — bugs détectés

Repo audité : `Stupnikjs/rust_liq` (master, clone local). ~4150 lignes Rust sur `eth-core`, `connector`, `morpho-api-graph`, `worker`.

---

## 🔴 CRITIQUE — plantage / risque direct sur les fonds ou la disponibilité

### 1. `snapshot()` retourne `None` sur un marché à 0 position → panics en cascade
**`worker/src/cache/mod.rs:110-125`**

```rust
pub fn snapshot(&self, id: FixedBytes<32>) -> Option<MarketSnapshot> {
    ...
    if market.positions.len() == 0 { 
        return None;
    }
    ...
}
```

Ce choix de design ("un marché vide n'a pas de snapshot") est raisonnable en soi, mais **au moins 5 call-sites font `.expect(...)` dessus** en supposant qu'il y a toujours au moins une position :

- `insert_pos` (`cache/mod.rs:129`) → panic **dès la première position insérée** dans un marché neuf (le marché est forcément vide juste avant).
- `all_snapshots()` (`cache/mod.rs:169`) → panic si **n'importe quel** marché suivi n'a aucune position (cas très fréquent : la plupart des marchés Morpho sur Base n'ont pas d'emprunteur à tout instant).
- `recompute_all_hf` (`cache/sort.rs:9`) → appelé à **chaque tick** de chaque `MarketLoopConsumer::run()`. Un marché qui perd sa dernière position (liquidée, remboursée, retirée) plante ce task au tick suivant.
- `lowest_hf` (`cache/sort.rs:86`) — même piège, + un `positions[0]` non protégé.
- `quote_market` (`runner/quote.rs:33`) → si un marché a des positions API mais qu'elles sont toutes filtrées par `borrow_assets_usd > 1.0` (cf. bug #3 plus bas), `positions` finit vide et l'init du bot plante.

**Impact réel :** les tasks `MarketLoopConsumer` sont spawnées avec `tokio::spawn(lc.run())` **sans jamais garder le `JoinHandle`** (`runner/market.rs:117`). Un panic dans une de ces tasks est donc **totalement invisible** — pas de log, pas de retry, le marché arrête juste d'être surveillé pour le reste de la vie du process, silencieusement.

**Fix suggéré :** faire retourner un `MarketSnapshot` avec `positions: vec![]` plutôt que `None` quand le marché existe mais est vide (réserver `None` au cas "marché inconnu"), et garder les `JoinHandle` des `MarketLoopConsumer` dans un `JoinSet` supervisé pour logger/relancer en cas de panic.

---

### 2. HF calculé à 0 quand l'oracle n'a pas encore répondu → liquidation de positions saines
**`worker/src/cache/refresh.rs:47-62`, `cache/sort.rs:8-30`, `morpho/mod.rs:61-79`**

`MarketStats::default()` initialise `oracle_price: U256::ZERO`. `onchain_oracle_refresh` peut échouer (RPC hiccup) et l'erreur est avalée :

```rust
let _ = self.cache.onchain_oracle_refresh(&self.connector, self.id).await;
self.cache.recompute_all_hf(self.id);
```

`recompute_all_hf` tourne quand même juste après, avec `oracle_price = 0`. Dans `hf()` :

```rust
let numerator = collateral_assets * oracle_price * lltv; // = 0 si oracle_price = 0
...
Some(numerator / denominator) // => HF = 0
```

Un HF à 0 est en dessous de `WAD` → `lowest_hf_and_interval` renvoie `interval = 0` → `try_liquidate` est déclenché **sur une position parfaitement saine**, simplement parce que l'oracle call a raté ce tick-là. La simulation (`call_raw` avant `send_tx`) devrait normalement bloquer l'envoi on-chain (la position n'est pas réellement liquidable), donc pas de perte de fonds directe — mais ça consomme les 20 tentatives du `spaming_map` pour cette adresse (bug #5) et gaspille du RPC en boucle.

**Fix suggéré :** ne pas recalculer/utiliser le HF tant que `oracle_price == 0` (marché "not ready"), et propager l'échec d'`onchain_oracle_refresh` au lieu de l'avaler avec `let _ =`.

---

### 3. `api_refresh` désactive (`canceled = true`) tout marché à ≤10 positions — **à l'encontre de la stratégie du bot**
**`worker/src/cache/refresh.rs:14-46`**

```rust
match fetch_all_positions(id, chain_id).await {
    Ok(positions) if positions.len() > 10 => { /* garde le marché */ }
    Ok(_) => { self.update(id, |m| m.canceled = true); }          // ≤10 positions
    Err(e) => { self.update(id, |m| m.canceled = true); }          // erreur API transitoire
}
```

`ids()` filtre tout marché `canceled` — et `canceled` n'est **jamais remis à `false` nulle part** (`grep canceled` confirme : 3 endroits qui le passent à `true`, zéro qui le repasse à `false`). Deux problèmes cumulés :

- Un marché avec 10 positions ou moins est **définitivement exclu**, alors que la stratégie documentée (`bugs.md`, mémoire projet) est justement de cibler les petits marchés à faible compétition. C'est probablement l'inverse du comportement voulu.
- Une simple erreur réseau transitoire sur le fetch API **blackliste le marché à vie** (pas de re-essai possible), alors qu'`api_refresh_loop(7200)` retente pourtant toutes les 2h — mais un succès ultérieur ne réinitialise pas `canceled`.

**Fix suggéré :** remettre `canceled = false` dans la branche de succès, et découpler "peu de positions" de "marché à ignorer" (ou au moins rendre le seuil configurable / le supprimer pour coller à la stratégie niche).

---

### 4. `MarketLoopConsumer` ne compile pas : `RwLock` non importé
**`worker/src/runner/market.rs:1-23,78`**

```rust
use std::sync::{Arc};   // pas de RwLock
...
pub struct MarketLoopConsumer {
    ...
    route_cache: Arc<RwLock<RouteCache>>,   // RwLock non résolu
    ...
}
...
let route = self.route_cache.read().unwrap().get_edge(&pos.market_id).cloned();
```

Erreur de compilation immédiate (déjà notée dans `bugs.md`, toujours présente). Par ailleurs le `.read().unwrap()` (synchrone, non `.await`) indique un usage `std::sync::RwLock`, cohérent avec le type dans `Runner` (`mod.rs` importe bien `std::sync::{Arc, RwLock}`) — il suffit donc d'ajouter le même import ici. La migration vers `tokio::sync::RwLock` mentionnée en commentaire (`// remplacer le std RwLock par tokio RwLock`) n'est elle pas faite, ce qui est OK à court terme mais à trancher.

**Fix :** `use std::sync::{Arc, RwLock};` dans `runner/market.rs`.

---

## 🟠 IMPORTANT — logique métier incorrecte

### 5. `compute_seized_asset` inverse `total_borrow_assets` et `total_borrow_shares`
**`worker/src/morpho/mod.rs:45-55`**

```rust
pub fn borrow_assets_from_shares(pos_shares: U256, tot_shares: U256, tot_borrow_assets: U256) -> U256 { ... }

pub fn compute_seized_asset(
    borrow_shares: U256,
    total_borrow_assets: U256,
    total_borrow_shares: U256,
    lltv: U256,
) -> U256 {
    let repay_assets = borrow_assets_from_shares(borrow_shares, total_borrow_assets, total_borrow_shares);
    //                                                            ^^^^^^^^^^^^^^^^^^  ^^^^^^^^^^^^^^^^^^
    //                                                            inversés vs la signature (tot_shares, tot_borrow_assets)
    ...
}
```

`total_borrow_assets` et `total_borrow_shares` sont passés dans le mauvais ordre par rapport à la signature de `borrow_assets_from_shares`. Ça donnerait un `repay_assets` complètement faux (shares et assets n'ont pas la même échelle).

**Bonne nouvelle :** `compute_seized_asset` n'est **appelée nulle part** dans le reste du code — c'est actuellement du code mort. Mais si tu comptes t'en servir pour dimensionner les liquidations, corrige l'ordre des arguments avant.

### 6. Le dimensionnement réel de la liquidation ignore la dette — tente de saisir 100% du collatéral
**`worker/src/liquidate/mod.rs:27-33`**

```rust
let mut seized_assets = pos.collateral_assets; // ajust with slippage
if seized_assets > wc_amount_in {
    seized_assets = wc_amount_in;
}
let calldata = encode_liquidate(&mparam, pos.address, seized_assets, U256::ZERO, steps, U256::ZERO);
```

Le montant à saisir n'est jamais calculé à partir de la dette réelle (assets empruntés + `liquidation_incentive_factor`) — la fonction dédiée à ça (`compute_seized_asset`, bug #5) n'est même pas appelée ici. Le code prend tout le collatéral, borné uniquement par la capacité du swap (`wc_amount_in`). Pour une liquidation partielle (position avec du collatéral en excès par rapport à la dette + incentive), ça va demander à saisir plus que ce que Morpho autorise et la transaction va très probablement revert en simulation (donc pas de perte de fonds, mais l'opportunité est ratée) — sauf coïncidence où la position est déjà proche de 100% de mauvaise dette.

**Fix suggéré :** calculer `seized_assets` à partir de `compute_seized_asset(pos.borrow_shares, total_borrow_assets, total_borrow_shares, lltv)` (une fois le bug #5 corrigé), puis appliquer le cap de slippage.

### 7. `update_borrow` / `update_repay` ne recalculent pas le HF
**`worker/src/cache/events.rs:56-77`**

`update_supply_collateral` et `update_withdraw_collateral` appellent `self.recompute_all_hf(market_id)` après mise à jour — mais **`update_borrow` et `update_repay` ne le font pas**. Après un `Borrow` (qui augmente directement le risque) ou un `Repay`, le `cached_hf` de la position reste périmé jusqu'au prochain tick de `refresh_oracle_and_hf` (qui recalcule tout de toute façon, donc l'impact est amorti par la fréquence des ticks) — mais l'ordre de tri (`sort_by_hf`) n'est lui aussi rafraîchi qu'au tick `% 10`. Une grosse position qui vient d'emprunter et de passer sous le LLTV peut donc rester invisible (pas en position 0 après tri) jusqu'à plusieurs ticks plus tard.

**Fix suggéré :** ajouter `self.recompute_all_hf(market_id)` dans `update_borrow` et `update_repay`, comme pour les deux autres handlers.

### 8. `update_liquidate` supprime la position même sur liquidation partielle
**`worker/src/cache/events.rs:90-96`**

```rust
pub fn update_liquidate(&self, log: &Log) {
    ...
    self.update(market_id, |m| {
        m.positions.retain(|p| p.address != borrower);
    });
}
```

Morpho Blue autorise les liquidations partielles (`seizedAssets`/`repaidShares` inférieurs au total). Ici, **n'importe quel** événement `Liquidate` sur une adresse retire complètement sa position du cache — même si elle a encore de la dette et du collatéral après une liquidation partielle faite par un concurrent. Cette position ne sera retrouvée qu'au prochain `api_refresh` (toutes les 2h par défaut), ce qui peut faire rater une deuxième liquidation entre-temps.

**Fix suggéré :** décoder `repaidShares`/`seizedAssets` du log et mettre à jour la position au lieu de la supprimer, ou déclencher un refresh ciblé de cette position.

### 9. `spaming_map` bloque une adresse définitivement après 20 échecs
**`worker/src/runner/market.rs:76-87`**

```rust
let attempts = self.spaming_map.entry(pos.address).or_default();
if *attempts < 20 {
    *attempts += 1;
    liquidate::liquidate(...).await;
}
```

Une fois `attempts == 20`, cette adresse n'est **plus jamais retentée**, pour toute la durée de vie du process (le `HashMap` n'est jamais purgé ni les compteurs réinitialisés en cas de succès ou de changement de HF). Le `bugs.md` du repo propose déjà un backoff exponentiel plutôt qu'un cutoff dur — cette version-ci a bien un plafond, mais c'est un plafond permanent plutôt qu'un ralentissement temporaire, donc une vraie opportunité de liquidation peut être perdue pour de bon si elle échoue 20 fois pour une raison transitoire (congestion, RPC, slippage ponctuel).

---

## 🟡 MOYEN — robustesse / disponibilité

### 10. Un panic dans une task critique ne fait pas mourir le process → systemd ne relance jamais
**`worker/src/runner/mod.rs:137-158`**

`tokio::join!` attend que les 4 handles (`log_handle`, `market_handle`, `refresh_handle`, `sub_handle`) soient toutes terminées, et logue chaque erreur individuellement — mais si une seule panique (ex: `market_handle`), les 3 autres continuent de tourner indéfiniment et **le process ne se termine jamais**. Résultat : `systemd` avec `Restart=on-failure` (mentionné dans le contexte projet) ne se déclenche jamais puisque le process reste "vivant", alors que la boucle de liquidation, elle, est bel et bien morte et silencieuse.

**Fix suggéré :** sur erreur d'une task jugée critique (`market_handle`, `sub_handle`), soit `std::process::exit(1)` explicitement, soit restructurer avec `tokio::select!` pour sortir dès qu'une task meurt.

### 11. Writer SQLite du backtest s'arrête définitivement après une seule erreur d'écriture
**`worker/src/backtest/mod.rs:61-91`**

Sur la première erreur d'écriture ou de jointure de task, la boucle `while let Some(batch) = rx.recv().await` fait `break` et le writer s'arrête pour de bon (`eprintln!("backtest_store: writer arrêté")`). Tous les `push_snapshot` suivants échouent silencieusement (l'erreur est avalée par `let _ =` dans `market.rs:63`). Pas de risque pour les fonds, mais perte silencieuse de toutes les données de backtest après le premier incident SQLite.

### 12. `Connector::call_raw` avec adresse `from` hardcodée
**`connector/src/lib.rs:29-36,145-157`**

```rust
.from(address!("78D3FEc647f35E5D413597D217C5E0D9605acE3E"))  // change asap
```

Commentaire "change asap" toujours présent — l'adresse `from` utilisée pour les simulations `eth_call` est en dur. Si cette adresse n'a pas les tokens/allowances nécessaires sur certains simulateurs stricts (ou si le nœud fait des vérifications de solde), ça peut fausser certaines simulations. À part ça, notez que `CallRaw` (le trait) et `Connector::call_raw` (méthode inhérente) ont un corps **identique et dupliqué** — sans impact fonctionnel (la méthode inhérente prime), mais à nettoyer.

### 13. `QuoteConsumer::quote_market` reconstruit un `UniswapV3` identique à chaque itération
**`worker/src/runner/quote.rs:27-31`**

Pas un bug fonctionnel, mais `UniswapV3::new(...)` avec les mêmes paramètres est recréé à chaque itération de la boucle `for id in self.cache.ids()` au lieu d'être sorti de la boucle — inefficace pour rien.

---

## 🟢 MINEUR / cosmétique

- `worker/src/runner/market.rs:67-69` : `if now - last_sec > 100 { last_sec = now; }` ne conditionne plus rien d'autre — logique morte (probablement un throttle de log/métrique jamais branché).
- `worker/src/cache/events.rs:113` : le commentaire `// data: [caller (32), assets (32)]` sur `update_supply_collateral` est trompeur — `caller` est indexé (topics[2]) donc absent de `data`, qui ne contient que `assets` à l'offset 0. Le code est correct, seul le commentaire induit en erreur.
- `EVENTS_SIG` (`runner/mod.rs:22-30`) souscrit à `Supply(...)` mais `process_log` (`cache/events.rs:20-44`) ne le traite jamais (`_ => {}`) — sans impact sur le HF (Supply ne touche pas un emprunteur), mais si tu veux exploiter les données de liquidité totale du marché plus tard, ce n'est pas branché.

---

## ✅ Ce qui est correct (contrairement à ce qu'on pouvait craindre)

Après vérification ligne à ligne contre `EventsLib.sol` de Morpho Blue (topics indexés vs `data`), le décodage ABI des events dans `cache/events.rs` est **correct** pour `Borrow`, `Repay`, `Liquidate`, `AccrueInterest`, `SupplyCollateral`, `WithdrawCollateral` (positions des topics et offsets `data` bons). La formule `hf()` dans `morpho/mod.rs` est dimensionnellement cohérente (résultat bien scalé en WAD). La gestion des nonces dans `connector/src/tx_sender.rs` (gap reuse, `NonceGuard` avec `Drop`, distinction `Mined/StillPending/Dropped/Unknown`) est solide. La reconnexion WS (`runner/mod.rs`) et la souscription à `SupplyCollateral` sont déjà en place — les deux points notés comme problématiques dans le contexte projet semblent donc déjà résolus dans cette version.

---

## Priorité de correction suggérée

1. **#4** (compile fix trivial, bloque tout le reste)
2. **#1** (panics en cascade — le plus dangereux pour la disponibilité)
3. **#3** (exclusion des petits marchés — contraire à la stratégie)
4. **#2** (HF=0 fantôme sur échec oracle)
5. **#6/#5** (dimensionnement des liquidations)
6. Le reste, par ordre de trafic (7, 8, 9, 10...)