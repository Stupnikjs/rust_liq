# rsliq — Recap audit bugs d'exécution (repo: Stupnikjs/rsliq)

Analyse effectuée par lecture de code + vérification de l'ABI Morpho Blue réelle
(EventsLib.sol). Focus uniquement sur les bugs qui menacent l'exécution
(panics, corruption de données, blocages, fuites mémoire) — pas les choix de
design.

---

## 1. [CRITIQUE] Panic garanti quand un marché passe à 0 positions

**Fichiers concernés :**
- `src/cache/mod.rs` : `snapshot()`, `insert_pos()`, `remove_pos()`, `all_snapshots()`
- `src/cache/sort.rs` : `recompute_all_hf()`, `lowest_hf()`
- `src/runner/market_loop.rs:40`

**Problème :**
`MarketCache::snapshot()` retourne `None` si `market.positions.len() == 0` :
```rust
if market.positions.len() == 0 { 
    return None;
}
```
Mais tous les appelants font `.expect(...)` sans gérer le `None` :
```rust
let snap = self.cache.snapshot(self.id).expect("snap not found"); // market_loop.rs:40
```
`ids()` filtre seulement sur `!canceled`, pas sur positions vides. Dès qu'un
marché atteint 0 position (tous les borrowers liquidés/remboursés — fréquent
sur les petits marchés ciblés par le bot), le prochain tick de
`MarketLoopConsumer::run()` panique.

**Impact :**
- La task `tokio::spawn(lc.run())` de ce marché meurt **silencieusement**
  (pas de `JoinHandle` tracké) → le marché n'est plus jamais monitoré.
- Le endpoint HTTP `GET /logs/{pair}` (via `all_snapshots()`) crash
  (500) dès qu'un seul marché a 0 positions.

**Fix à faire :**
- Faire retourner un `MarketSnapshot` avec `positions: vec![]` au lieu de
  `None` sur positions vides (ou gérer le `None` proprement dans chaque
  appelant avec un `continue`/retour par défaut au lieu de `.expect`).
- Ajouter un tracking de `JoinHandle` par marché dans `Runner::market_loop`
  pour détecter/relancer une task qui meurt.

---

## 2. [CRITIQUE] Mauvais topics/offsets dans le décodage des logs on-chain

**Fichier concerné :** `src/cache/events.rs`

Vérifié contre l'ABI réelle de Morpho Blue (`EventsLib.sol`) :

| Event       | Signature réelle (indexed)                    | Code actuel        | Bug                                  |
|-------------|-----------------------------------------------|---------------------|---------------------------------------|
| `Repay`     | `id, caller(idx), onBehalf(idx)` → onBehalf = `topics[3]` | lit `topics[2]` | lit **caller** au lieu de `onBehalf` |
| `Liquidate` | `id, caller(idx), borrower(idx)` → borrower = `topics[3]` | lit `topics[2]` | lit **caller** au lieu de `borrower` |
| `Borrow`    | data = `[caller, assets, shares]` → shares = offset **64** | lit offset `32` | lit **assets** au lieu de `shares`   |

**Impact :**
- `update_repay` décrémente les `borrow_shares` du mauvais compte.
- `update_liquidate` retire le mauvais borrower du cache → une position
  réellement liquidée reste visible avec un HF stale, risque de retenter une
  liquidation sur une position qui n'existe plus.
- `update_borrow` ajoute la valeur `assets` (pas `shares`) à `borrow_shares`
  → HF faux pour toute position ayant emprunté après le refresh initial.

**Fix à faire :**
- `update_repay` : lire `on_behalf` depuis `log.topics()[3]` (pas `[2]`).
- `update_liquidate` : lire `borrower` depuis `log.topics()[3]` (pas `[2]`).
- `update_borrow` : lire `shares` à l'offset `64` dans `log.data().data`
  (pas `32`, qui correspond à `assets`).
- Idéalement, ajouter un test unitaire par event avec un log réel
  (topics + data hexadécimaux capturés depuis Basescan) pour verrouiller
  ces offsets.

---

## 3. [MAJEUR] `SupplyCollateral` jamais câblé — cache de collatéral jamais rafraîchi

**Fichiers concernés :** `src/cache/events.rs`, `src/connector/mod.rs`

`update_supply_collateral` existe (events.rs) mais :
- N'est pas dans le filtre WS (`connector/mod.rs`, liste `.events([...])` ne
  contient pas `"SupplyCollateral(bytes32,address,address,uint256)"`).
- N'est pas dispatché dans `process_log` (le `match` ne couvre que
  `Borrow`/`Repay`/`Liquidate`/`AccrueInterest`).

**Impact :**
Tout ajout de collatéral après le refresh initial (`api_refresh`) est
invisible pour le bot en continu → HF de plus en plus faux avec le temps
pour toute position augmentant son collatéral. Les nouvelles positions
créées uniquement via `SupplyCollateral` avant tout `Borrow` ne sont jamais
insérées dans le cache.

Bonus : si l'event est branché sans corriger l'offset, `read_u256(&log.data().data, 32)`
va paniquer (`SupplyCollateral` n'a qu'un seul mot de data — `assets` à
l'offset 0 — pas de données à l'offset 32).

**Fix à faire :**
1. Ajouter `"SupplyCollateral(bytes32,address,address,uint256)"` à la liste
   `.events([...])` dans `connector/mod.rs`.
2. Ajouter le match arm correspondant dans `process_log` :
   ```rust
   x if *x == keccak256("SupplyCollateral(bytes32,address,address,uint256)") => {
       self.update_supply_collateral(log);
   }
   ```
3. Corriger `update_supply_collateral` pour lire `on_behalf` sur
   `topics[3]` (pas `topics[2]`, même bug que #2) et `assets` à l'offset `0`
   (pas `32`).

---

## 4. [CRITIQUE] Nonce jamais libéré après échec d'envoi de tx — bloque le bot définitivement

**Fichier concerné :** `src/connector/tx_sender.rs::send_tx`

```rust
let gas_limit = match http.estimate_gas(tx_req).await {
    Ok(g) => g,
    Err(e) => { self.release_nonce(nonce); return Err(e.into()); } // seul cas de release
};
// ...
let pending = http.send_raw_transaction(&buf).await?;   // pas de release_nonce si erreur
let receipt = pending.get_receipt().await?;              // pas de release_nonce si erreur/timeout
```

Le nonce n'est libéré **que** si `estimate_gas` échoue. Si
`send_raw_transaction` échoue (RPC down, tx rejetée) ou si `get_receipt`
échoue/timeout (tx droppée, remplacée, reorg), le nonce reste consommé sans
transaction confirmée correspondante.

**Impact :**
Le prochain `send_tx` prend `nonce+1`, qui restera bloqué indéfiniment en
mempool (Ethereum exige des nonces séquentiels par compte). **Après le
premier échec de ce type, le bot ne peut plus jamais envoyer de
transaction** tant que le nonce n'est pas resynchronisé manuellement.

Aggravé par le fait que `liquidate::liquidate` (`src/liquidate/mod.rs`)
ignore le résultat de l'envoi :
```rust
let tx_hash = conn.send_tx(liquidator_addr, calldata).await; // jamais inspecté
```
→ ce blocage est invisible en prod, aucun log d'erreur.

**Fix à faire :**
- Appeler `self.release_nonce(nonce)` dans **tous** les chemins d'erreur de
  `send_tx` (après `send_raw_transaction` et après `get_receipt`), pas
  seulement après `estimate_gas`.
- Dans `liquidate::liquidate`, matcher le résultat de `send_tx` et logger
  systématiquement les erreurs (`eprintln!` minimum, ou mieux : incrémenter
  une métrique/compteur d'échecs consultable).
- Envisager un timeout explicite sur `get_receipt()` pour éviter un hang
  indéfini bloquant le `MarketLoopConsumer::run()` du marché concerné.

---

## 5. [MAJEUR] Batch de backtest jamais vidé — persistance qui s'arrête + fuite mémoire

**Fichier concerné :** `src/runner/market_loop.rs`

```rust
batch.extend_from_slice(to_push_in_batch.as_slice());
if batch.len() == 32 {
    let _ = self.backest.push_snapshot(&batch).await;
}
```

`batch` n'est jamais vidé après le push. `snap_to_4_batch` ajoute 4 éléments
par tick → `batch.len()` vaut exactement 32 une seule fois (au 8ᵉ tick),
déclenche un push, puis continue de grossir (36, 40, 44…) sans jamais
retomber à 32.

**Impact :**
- Le SQLite ne reçoit qu'**un seul batch** pendant toute la durée de vie du
  process.
- `batch` grossit indéfiniment en mémoire — fuite mémoire lente, process
  tournant en continu sous systemd (`Restart=on-failure`) → OOM à terme.

**Fix à faire :**
```rust
batch.extend_from_slice(to_push_in_batch.as_slice());
if batch.len() >= 32 {                      // >= plutôt que == par sécurité
    let _ = self.backest.push_snapshot(&batch).await;
    batch.clear();                           // <- manquant actuellement
}
```

---

## Résumé priorisé

| # | Sévérité  | Bug                                                        | Fichier(s)                          |
|---|-----------|--------------------------------------------------------------|--------------------------------------|
| 1 | Critique  | Panic sur marché à 0 positions, task meurt silencieusement   | cache/mod.rs, cache/sort.rs, market_loop.rs |
| 2 | Critique  | Mauvais topics/offsets Repay/Liquidate/Borrow → données corrompues | cache/events.rs                |
| 3 | Majeur    | SupplyCollateral jamais souscrit ni dispatché                | events.rs, connector/mod.rs         |
| 4 | Critique  | Nonce jamais libéré après échec d'envoi → bot bloqué          | connector/tx_sender.rs               |
| 5 | Majeur    | Batch backtest jamais vidé → 1 seul push + fuite mémoire      | runner/market_loop.rs                |