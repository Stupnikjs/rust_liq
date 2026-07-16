#![allow(dead_code, unused_variables, unused_imports)]
use alloy_primitives::{Address, FixedBytes, U256, address, Bytes};

// Constante vérifiée à la compilation
pub const IRM: Address = address!("0x46415998764C29aB2a25CbeA6254146D50D22687");

#[derive(Debug, Clone, Default)]
pub struct MarketParam {
    pub id: FixedBytes<32>,
    pub loan_token: Address,
    pub collateral_token: Address,
    pub oracle: Address,
    pub irm: Address,
    pub lltv: U256, // Remplacé par U256
    pub loan_token_str: String,
    pub collateral_token_str: String,
    pub chain_id: u32,
    pub loan_token_decimals: u16,
    pub collateral_token_decimals: u16,
}

#[derive(Debug, Clone, Default)]
pub struct MarketContractParams {
    pub loan_token: Address,
    pub collateral_token: Address,
    pub oracle: Address,
    pub irm: Address,
    pub lltv: U256, // Remplacé par U256
}

impl  MarketContractParams {
    pub fn to_bytes(&self) -> Bytes {
        let mut market_bytes:Vec<u8> = Vec::with_capacity(32*5); 
        market_bytes.extend_from_slice(&[0u8; 12]);
        market_bytes.extend_from_slice(self.loan_token.as_slice()); 
        market_bytes.extend_from_slice(&[0u8; 12]);
        market_bytes.extend_from_slice(self.collateral_token.as_slice()); 
        market_bytes.extend_from_slice(&[0u8; 12]);
        market_bytes.extend_from_slice(self.oracle.as_slice()); 
        market_bytes.extend_from_slice(&[0u8; 12]);
        market_bytes.extend_from_slice(self.irm.as_slice()); 
         
        market_bytes.extend_from_slice(&self.lltv.to_be_bytes::<32>());
        market_bytes.into()
    }
    
}


impl MarketParam {
    pub fn to_market_contract_params(&self) -> MarketContractParams {
        MarketContractParams {
            loan_token: self.loan_token,
            collateral_token: self.collateral_token,
            oracle: self.oracle,
            irm: self.irm,
            lltv: self.lltv, // Copie directe (U256 implémente Copy/Clone)
        }
    }

    pub fn is_eth_correlated(&self) -> bool {
        self.collateral_token_str.contains("ETH") && self.loan_token_str.contains("ETH")
    }

    pub fn is_btc_correlated(&self) -> bool {
        self.collateral_token_str.contains("BTC") && self.loan_token_str.contains("BTC")
    }

    pub fn get_pair(&self) -> String {
        format!("{}/{}", self.collateral_token_str, self.loan_token_str)
    }

    pub fn get_collateral_token(&self) -> Address {
        self.collateral_token
    }

    pub fn get_loan_token(&self) -> Address {
        self.loan_token
    }

    pub fn get_lltv(&self) -> U256 {
        self.lltv
    }

    pub fn max_slippage(&self) -> f64 {
    let lltv_pct = self.lltv.to::<u128>() as f64 / 1e18;
    100.0 - lltv_pct * 100.0
}
}



pub fn price_normalized(loan_dec:u16, coll_dec:u16, oracle_price:U256 ) -> f64 {
    let dec = 36 + loan_dec as u32 - coll_dec as u32 ;      
    oracle_price.to_string().parse::<f64>().unwrap_or(0.0) / 10_f64.powi(dec as i32)
}