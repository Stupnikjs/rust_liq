use alloy_primitives::{FixedBytes, U256};
use crate::cache::{MarketCache, positions::BorrowPosition};
use morpho::utils::WAD;
use eth_core::utils::BoxError;

// hf en dessous duquel une position est de la bad debt (pas rentable à liquider,
// ou données corrompues) -> ignorée pour le ciblage, gardée dans le snapshot pour le backtest.
const HF_IGNORE_THRESHOLD_NUM: u64 = 4;
const HF_IGNORE_THRESHOLD_DEN: u64 = 10;

const MIN_INTERVAL_SECS: f64 = 2.0;
const MAX_INTERVAL_SECS: f64 = 14_400.0; // ~3h27, ou 14_400 (4h) ?


const SCALE_CORRELATED: f64 = 0.003;
const SCALE_UNCORRELATED: f64 = 0.02;

fn hf_ignore_threshold() -> U256 {
    WAD * U256::from(HF_IGNORE_THRESHOLD_NUM) / U256::from(HF_IGNORE_THRESHOLD_DEN)
}

fn hf_to_f64(hf: U256) -> f64 {
    (hf.to::<u128>() as f64) / 1e18 // perte de précision négligeable, usage scheduling only
}

// exponentielle saturante : interval = MIN + (MAX-MIN)*(1 - e^(-margin/scale))
// évite l'effet falaise d'une table de paliers (petit delta de hf -> gros saut d'interval)
fn interval_from_hf(hf: U256, is_correlated: bool) -> u64 {
    let margin = hf_to_f64(hf) - 1.0;
    if margin <= 0.0 {
        return 0; // sous le seuil, action immédiate
    }

    let scale = if is_correlated { SCALE_CORRELATED } else { SCALE_UNCORRELATED };
    let raw = MIN_INTERVAL_SECS
        + (MAX_INTERVAL_SECS - MIN_INTERVAL_SECS) * (1.0 - (-margin / scale).exp());

    raw.round() as u64
}

impl MarketCache {
    pub fn recompute_all_hf(&self, id: FixedBytes<32>) -> Result<(), BoxError> {
        let Some(snap) = self.snapshot(id) else {
            println!("snapshot failed for {}", id.to_string());
            return Ok(()); // pas de panic, le market_loop continue son cycle
        };
        let Some(mparam) = self.get_market_param_by_id(id) else {
            println!("get market params failed for {}", id.to_string());
            return Ok(());
        };

        let updated: Vec<BorrowPosition> = snap
            .positions
            .iter()
            .map(|p| {
                let mut new_pos = p.clone();
                new_pos.cached_hf = p.health_factor(
                    snap.stats.total_borrow_assets,
                    snap.stats.total_borrow_shares,
                    mparam.lltv,
                    snap.stats.oracle_price,
                );
                new_pos
            })
            .collect();

        _ = self.update(id, |m| {
            m.positions = updated;
        });
        Ok(())
    }


    pub fn sort_by_hf(&self, id: FixedBytes<32>) -> Result<(), BoxError> {
        _ = self.update(id, |m| {
            m.positions.sort_by(|a, b| match (a.cached_hf, b.cached_hf) {
                (Some(a_hf), Some(b_hf)) => a_hf.cmp(&b_hf),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            });
        });
        Ok(())
    }

    pub fn lowest_hf_and_interval(&self, id: FixedBytes<32>, is_correlated: bool) -> (Option<BorrowPosition>, u64) {
        let Some(snap) = self.snapshot(id) else {
            return (None, 3600);
        };

        let threshold = hf_ignore_threshold();

        
        let Some(first) = snap
            .positions
            .iter()
            .find(|p| p.cached_hf.map_or(false, |hf| hf >= threshold))
            .cloned()
        else {
            return (None, 3600);
        };

        let hf = first.cached_hf.expect("filtré ci-dessus, toujours Some ici");
        (Some(first), interval_from_hf(hf, is_correlated))
    }

    pub fn lowest_hf(&self, id: FixedBytes<32>) -> Option<BorrowPosition> {
        let threshold = hf_ignore_threshold();
        self.snapshot(id)?
            .positions
            .iter()
            .find(|p| p.cached_hf.map_or(false, |hf| hf >= threshold))
            .cloned()
    }
}
