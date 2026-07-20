use std::fs;
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

use alloy_primitives::{FixedBytes, U256};

use serde::{Serialize, de};
use morpho::utils::{WAD, hf_to_f64};
use morpho::types::price_normalized;   
use super::{MarketSnapshot, MarketCache};

/// Largeur de chaque tranche de l'histogramme, en points de pourcentage (5 => 0.05 en HF)
const HF_HISTOGRAM_STEP_PCT: u64 = 5;
/// Nombre de tranches entre 1.00 et 1.50 (10 * 5% = 50%)
const HF_HISTOGRAM_BUCKETS: usize = 10;

#[derive(Serialize)]
pub struct MarketLog {
    pair: String,
    market_id: String,
    ts: u64,
    total_positions: usize,
    stale_positions: usize,
    /// Positions déjà en dessous de 1.00 (liquidables immédiatement)
    hf_below_100: usize,
    /// Histogramme de 1.00 à 1.50, par pas de 0.05.
    /// index 0 = [1.00, 1.05), index 1 = [1.05, 1.10), ..., index 9 = [1.45, 1.50)
    hf_histogram: [usize; HF_HISTOGRAM_BUCKETS],
    /// Positions au-dessus ou égales à 1.50 (considérées safe)
    hf_above_150: usize,
    total_borrowed_collateral_equiv: String,
    collateral_at_risk_20pct: String,
    price_normalized: f64,
    closest_to_liquidation_addr: Option<String>,
    closest_to_liquidation_hf: Option<f64>,
    closest_to_liquidation_collateral: Option<String>,
}


pub fn id_to_market_log(cache: &MarketCache, id: FixedBytes<32>) -> MarketLog {
    let snap = cache.snapshot(id).expect("snapshot failed in id_to_market_log");
    snap_to_market_log(&snap)
}

pub fn snap_to_market_log(snapshot: &MarketSnapshot) -> MarketLog {
    let pair = snapshot.params.get_pair().to_string();
    let ts = now_ms();

    let threshold_20 = WAD * U256::from(120u64) / U256::from(100u64);
    let threshold_150 = WAD * U256::from(150u64) / U256::from(100u64);

    let mut hf_below_100 = 0usize;
    let mut hf_histogram = [0usize; HF_HISTOGRAM_BUCKETS];
    let mut hf_above_150 = 0usize;
    let mut stale = 0usize;
    let mut collateral_at_risk = U256::ZERO;

    for pos in &snapshot.positions {
        match pos.cached_hf {
            Some(hf) => {
                // métrique de risque indépendante de l'affichage histogramme
                if hf < threshold_20 {
                    collateral_at_risk = collateral_at_risk.saturating_add(pos.collateral_assets);
                }

                if hf < WAD {
                    hf_below_100 += 1;
                } else if hf >= threshold_150 {
                    hf_above_150 += 1;
                } else {
                    // diff < 0.5 * WAD ici, donc diff_pct < 50, aucun risque d'overflow u64
                    let diff = hf - WAD;
                    let diff_pct = (diff * U256::from(100u64) / WAD).to::<u64>();
                    let idx = (diff_pct / HF_HISTOGRAM_STEP_PCT) as usize;
                    hf_histogram[idx.min(HF_HISTOGRAM_BUCKETS - 1)] += 1;
                }
            }
            None => stale += 1,
        }
    }

    let closest = snapshot.positions.iter()
        .filter_map(|p| p.cached_hf.map(|hf| (p, hf)))
        .min_by_key(|(_, hf)| *hf);

    let (closest_addr, closest_hf, closest_collateral) = match closest {
        Some((p, hf)) => (Some(p.address.to_string()), Some(hf_to_f64(hf)), Some(p.collateral_assets.to_string())),
        None => (None, None, None),
    };

    let e36 = U256::from(10u64).pow(U256::from(36u64));
    let total_borrowed_collateral_equiv = snapshot
        .stats
        .total_borrow_assets
        .checked_mul(e36)
        .and_then(|v| v.checked_div(snapshot.stats.oracle_price))
        .unwrap_or(U256::ZERO);

    let price_normalized = price_normalized(
        snapshot.params.loan_token_decimals,
        snapshot.params.collateral_token_decimals,
        snapshot.stats.oracle_price,
    );

    MarketLog {
        pair,
        market_id: hex::encode(snapshot.id),
        ts,
        total_positions: snapshot.positions.len(),
        stale_positions: stale,
        hf_below_100,
        hf_histogram,
        hf_above_150,
        total_borrowed_collateral_equiv: total_borrowed_collateral_equiv.to_string(),
        collateral_at_risk_20pct: collateral_at_risk.to_string(),
        price_normalized,
        closest_to_liquidation_addr: closest_addr,
        closest_to_liquidation_hf: closest_hf,
        closest_to_liquidation_collateral: closest_collateral,
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}