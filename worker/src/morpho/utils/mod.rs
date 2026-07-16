use alloy_primitives::U256; 

pub const WAD: U256 = U256::from_limbs([1_000_000_000_000_000_000, 0, 0, 0]);

pub fn hf_to_f64(hf: U256) -> f64 {
    hf.to_string().parse::<f64>().unwrap_or(0.0) / 1e18
}