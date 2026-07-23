use eth_core::utils::BoxError;
use rusqlite::{params, Connection};
use serde::Serialize;
use tokio::sync::mpsc;

use crate::{cache::MarketSnapshot, swap::now_ms};

#[derive(Debug, Clone, Serialize)]
pub struct BacktestSnapshot {
    pub ts: u64,
    pub market_id: String,
    pub oracle_price: String,
    pub lltv: String,
    pub loan_token_decimals: u16,
    pub collateral_token_decimals: u16,
    pub borrower: String,
    pub collateral_assets: String,
    pub borrow_shares: String,
    pub total_borrow_assets: String,
    pub total_borrow_shares: String,
}

pub struct BacktestStore {
    tx: mpsc::Sender<Vec<BacktestSnapshot>>,
    db_path: String,
}

impl BacktestStore {

    // creation de la table 
    // loop sur le receiver du channel rx et expose tx pour push 
    // retourne la connection sql 
    pub async fn new(db_path: &str) -> anyhow::Result<Self> {
        let db_path = db_path.to_string();
        let db_path_for_conn = db_path.clone();

        let conn = tokio::task::spawn_blocking(move || -> anyhow::Result<Connection> {
            let conn = Connection::open(&db_path)?;
            conn.execute_batch(
                "PRAGMA journal_mode=WAL;
                    CREATE TABLE IF NOT EXISTS backtest_snapshots (
                    ts INTEGER NOT NULL,
                    market_id TEXT NOT NULL,
                    oracle_price TEXT NOT NULL,
                    lltv TEXT NOT NULL,
                    loan_token_decimals INTEGER NOT NULL,
                    collateral_token_decimals INTEGER NOT NULL,
                    borrower TEXT NOT NULL,
                    collateral_assets TEXT NOT NULL,
                    borrow_shares TEXT NOT NULL,
                    total_borrow_assets TEXT NOT NULL,
                    total_borrow_shares TEXT NOT NULL,
                    PRIMARY KEY (market_id, borrower, ts)
                );
                CREATE INDEX IF NOT EXISTS idx_backtest_ts ON backtest_snapshots(ts);"
            )?;
            Ok(conn)
        }).await??;

        let (tx, mut rx) = mpsc::channel::<Vec<BacktestSnapshot>>(32);

        tokio::spawn(async move {
            let mut conn = Some(conn);

            while let Some(batch) = rx.recv().await {
                let c = match conn.take() {
                    Some(c) => c,
                    None => {
                        eprintln!("backtest_store: pas de connexion valide, arrêt du writer");
                        break;
                    }
                };

                let res = tokio::task::spawn_blocking(move || {
                    Self::write_batch(c, batch) 
                }).await;

                match res {
                    Ok(Ok(c)) => conn = Some(c),
                    Ok(Err(e)) => {
                        eprintln!("backtest_store: échec écriture batch: {e}");
                        break; // <- conn reste None -> évite le E0382 à l'itération suivante
                    }
                    Err(e) => {
                        eprintln!("backtest_store: task jointure échouée: {e}");
                        break; // <- idem
                    }
                }
            }
            // 
            eprintln!("backtest_store: writer arrêté");
        });

        Ok(Self { tx, db_path:db_path_for_conn })
    }

    pub async fn push_snapshot(&self, batch: &Vec<BacktestSnapshot>) -> Result<(), BoxError> {
        let _ = self.tx.send(batch.to_vec()).await
            .map_err(|_| anyhow::anyhow!("backtest writer task fermée")); 
            Ok(())
    }

    fn write_batch(conn: Connection, batch: Vec<BacktestSnapshot>) -> anyhow::Result<Connection> {
        let tx = conn.unchecked_transaction()?;
        {
            let mut stmt = tx.prepare_cached(
                "INSERT OR REPLACE INTO backtest_snapshots
                (ts, market_id, oracle_price, lltv, loan_token_decimals, collateral_token_decimals,
                 borrower, collateral_assets, borrow_shares, total_borrow_assets, total_borrow_shares)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)"
            )?;
            for snap in &batch {
                stmt.execute(params![
                    snap.ts as i64, snap.market_id, snap.oracle_price, snap.lltv,
                    snap.loan_token_decimals, snap.collateral_token_decimals, snap.borrower,
                    snap.collateral_assets, snap.borrow_shares, snap.total_borrow_assets,
                    snap.total_borrow_shares,
                ])?;
            }
        }
        tx.commit()?;
        Ok(conn)
    }

    pub async fn get_snapshots(
        &self,
        market_id: String,
        limit: i64,
    ) -> anyhow::Result<Vec<BacktestSnapshot>> {
        let db_path = self.db_path.clone();

        tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<BacktestSnapshot>> {
            let conn = Connection::open_with_flags(
                &db_path,
                rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
            )?;

            let mut stmt = conn.prepare(
                "SELECT ts, market_id, oracle_price, lltv, loan_token_decimals,
                        collateral_token_decimals, borrower, collateral_assets,
                        borrow_shares, total_borrow_assets, total_borrow_shares
                 FROM backtest_snapshots
                 WHERE market_id = ?1
                 ORDER BY ts DESC
                 LIMIT ?2",
            )?;

            let rows = stmt.query_map(params![market_id, limit], |row| {
                Ok(BacktestSnapshot {
                    ts: row.get::<_, i64>(0)? as u64,
                    market_id: row.get(1)?,
                    oracle_price: row.get(2)?,
                    lltv: row.get(3)?,
                    loan_token_decimals: row.get(4)?,
                    collateral_token_decimals: row.get(5)?,
                    borrower: row.get(6)?,
                    collateral_assets: row.get(7)?,
                    borrow_shares: row.get(8)?,
                    total_borrow_assets: row.get(9)?,
                    total_borrow_shares: row.get(10)?,
                })
            })?;

            rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
        })
        .await?
    }
}




pub fn snap_to_4_batch(snap: &MarketSnapshot) -> Vec<BacktestSnapshot> {
    let lowest_hf = snap.positions.iter().take(4); 
    let mut snaps = Vec::with_capacity(4); 
    for p in lowest_hf {
        let backtest_snap = BacktestSnapshot{
            ts: now_ms(),
            market_id: p.market_id.to_string(),
            oracle_price: snap.stats.oracle_price.to_string(),
            lltv: snap.params.lltv.to_string(),
            collateral_assets: p.collateral_assets.to_string(),
            collateral_token_decimals: snap.params.collateral_token_decimals,
            loan_token_decimals: snap.params.loan_token_decimals,
            borrow_shares: p.borrow_shares.to_string(),
            total_borrow_assets: snap.stats.total_borrow_assets.to_string(),
            total_borrow_shares: snap.stats.total_borrow_shares.to_string(),
            borrower: p.address.to_string(),
        }; 
        snaps.push(backtest_snap);

    }

    snaps
}