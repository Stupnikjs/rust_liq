#!/usr/bin/env python3
"""
Compare les borrowers présents dans BacktestSnapshot (sqlite) avec les
liquidations récentes récupérées depuis l'API GraphQL de Morpho.
 
Usage:
    python3 check_liquidations.py --db data/backtest.db --chain-id 8453 --limit 500
    python3 check_liquidations.py --db data/backtest.db --table snapshots --hours 24
 
Dépendances: requests (pip install requests --break-system-packages)
"""
 
import argparse
import sqlite3
import sys
import time
from typing import Optional
 
import requests
 
MORPHO_API_URL = "https://api.morpho.org/graphql"
 
LIQUIDATIONS_QUERY = """
query RecentLiquidations($chainId: Int!, $first: Int!) {
  marketTransactions(
    first: $first
    orderBy: Timestamp
    orderDirection: Desc
    where: { chainId_in: [$chainId], type_in: [Liquidation] }
  ) {
    items {
      blockNumber
      timestamp
      txHash
      logIndex
      user {
        address
      }
      market {
        marketId
      }
      data {
        ... on MarketTransactionLiquidationData {
          seizedAssets
          repaidAssets
          badDebtAssets
          liquidator
        }
      }
    }
  }
}
"""
 
 
def fetch_liquidations(chain_id: int, first: int = 1000) -> list[dict]:
    resp = requests.post(
        MORPHO_API_URL,
        json={
            "query": LIQUIDATIONS_QUERY,
            "variables": {"chainId": chain_id, "first": first},
        },
        timeout=30,
    )
    resp.raise_for_status()
    payload = resp.json()
    if "errors" in payload:
        raise RuntimeError(f"Morpho API error: {payload['errors']}")
    return payload["data"]["marketTransactions"]["items"]
 
 
def load_snapshots(db_path: str, table: str, since_ts: Optional[int] = None) -> list[dict]:
    con = sqlite3.connect(db_path)
    con.row_factory = sqlite3.Row
    cur = con.cursor()
 
    cols = (
        "ts, market_id, oracle_price, lltv, loan_token_decimals, "
        "collateral_token_decimals, borrower, collateral_assets, "
        "borrow_shares, total_borrow_assets, total_borrow_shares"
    )
    query = f"SELECT {cols} FROM {table}"
    params = ()
    if since_ts is not None:
        query += " WHERE ts >= ?"
        params = (since_ts,)
    query += " ORDER BY ts DESC"
 
    cur.execute(query, params)
    rows = [dict(r) for r in cur.fetchall()]
    con.close()
    return rows
 
 
def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--db", required=True, help="Chemin vers le fichier sqlite (.db)")
    ap.add_argument("--table", default="backtest_snapshots", help="Nom de la table (default: backtest_snapshots)")
    ap.add_argument("--chain-id", type=int, default=8453, help="Chain ID Morpho (default: 8453 = Base)")
    ap.add_argument("--limit", type=int, default=1000, help="Nb max de liquidations à récupérer")
    ap.add_argument("--hours", type=float, default=None, help="Ne garder que les snapshots des N dernières heures")
    args = ap.parse_args()
 
    since_ts = None
    if args.hours is not None:
        since_ts = int(time.time() * 1000) - int(args.hours * 3600 * 1000)
 
    print(f"[*] Lecture des snapshots depuis {args.db} (table={args.table})...")
    snapshots = load_snapshots(args.db, args.table, since_ts)
    print(f"[*] {len(snapshots)} snapshots chargés.")
 
    if not snapshots:
        print("Aucun snapshot trouvé, rien à comparer.")
        return
 
    borrower_snapshots: dict[str, list[dict]] = {}
    for s in snapshots:
        addr = s["borrower"].lower()
        borrower_snapshots.setdefault(addr, []).append(s)
 
    print(f"[*] {len(borrower_snapshots)} borrowers distincts dans la db.")
 
    print(f"[*] Récupération des liquidations récentes sur chainId={args.chain_id}...")
    liquidations = fetch_liquidations(args.chain_id, args.limit)
    print(f"[*] {len(liquidations)} liquidations récupérées depuis l'API Morpho.")
 
    matches = []
    for liq in liquidations:
        addr = liq["user"]["address"].lower()
        if addr in borrower_snapshots:
            matches.append((addr, liq, borrower_snapshots[addr]))
 
    print()
    if not matches:
        print("Aucune correspondance trouvée entre tes snapshots et les liquidations récentes.")
        return
 
    print(f"=== {len(matches)} borrower(s) en snapshot ET liquidé(s) ===\n")
    for addr, liq, snaps in matches:
        data = liq.get("data") or {}
        print(f"Borrower: {addr}")
        print(f"  Market:      {liq['market']['marketId']}")
        print(f"  Tx:          {liq['txHash']}  (block {liq['blockNumber']})")
        print(f"  Seized:      {data.get('seizedAssets')}")
        print(f"  Repaid:      {data.get('repaidAssets')}")
        print(f"  Bad debt:    {data.get('badDebtAssets')}")
        print(f"  Liquidator:  {data.get('liquidator')}")
        print(f"  Snapshots pris pour ce borrower: {len(snaps)}")
        for s in snaps[:3]:
            print(f"    - ts={s['ts']} market={s['market_id']} collateral={s['collateral_assets']} borrow_shares={s['borrow_shares']}")
        if len(snaps) > 3:
            print(f"    ... (+{len(snaps) - 3} autres)")
        print()
 
 
if __name__ == "__main__":
    try:
        main()
    except requests.HTTPError as e:
        print(f"Erreur HTTP appel API Morpho: {e}", file=sys.stderr)
        sys.exit(1)
    except sqlite3.OperationalError as e:
        print(f"Erreur sqlite (vérifie --table et --db): {e}", file=sys.stderr)
        sys.exit(1)