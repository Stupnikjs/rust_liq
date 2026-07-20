use std::str::FromStr;
use alloy_primitives::{Address, FixedBytes, U256};
use morpho_api_graph::{fetch_all_market, types::{MarketItem, PositionItem}}; 
use crate::{cache::positions::BorrowPosition}; 
use morpho::types::MarketParam; 


pub fn market_item_to_morpho_market(item: &MarketItem, chain_id: u32) -> Result<MarketParam, anyhow::Error> {
    // 1. Convertir l'ID du marché (String hex) en FixedBytes<32> ou [u8; 32] selon ton type
    // Si ton MarketParams utilise FixedBytes<32> :
    let market_id = FixedBytes::<32>::from_str(&item.id)?;

    // 2. Convertir les adresses (String hex) en Address
    let loan_token = Address::from_str(&item.loan_asset.address)?;
    let collateral_asset = item
    .collateral_asset
    .as_ref()
    .ok_or_else(|| anyhow::anyhow!("no collasset found"))?; 
    let collateral_token = Address::from_str(&collateral_asset.address)?; 
    let oracle = Address::from_str(&item.oracle_address)?;
    let irm  = Address::from_str(&item.irm)?;

    // 3. Convertir le LLTV (ton type Number) en U256
    // Supposons que ton type Number ait une méthode pour récupérer un u128 ou un String
    let lltv_raw = item.lltv.parse_u128()?; // Utilise la méthode de ton type Number
    let lltv = U256::from(lltv_raw);
    let collateral_token_str = collateral_asset.symbol.clone(); 
    let collateral_token_decimals = collateral_asset.decimals as u16;
    
    let loan_token_str = item.loan_asset.symbol.clone();
    let loan_token_decimals = item.loan_asset.decimals as u16;
    // 4. Instancier et retourner ton struct MarketParams
    Ok(MarketParam {
        id: market_id,
        loan_token: loan_token,
        collateral_token: collateral_token ,
        oracle: oracle,
        irm: irm,
        lltv: lltv,
        chain_id: chain_id,
        collateral_token_str: collateral_token_str,
        collateral_token_decimals: collateral_token_decimals, 
        loan_token_decimals: loan_token_decimals,
        loan_token_str: loan_token_str,
    })
}






pub fn position_item_to_borrow_pos(
    pos_item: PositionItem, 
    market_id: FixedBytes<32>
) -> BorrowPosition {
    // 1. Convertir l'adresse String en type Address d'Alloy
    // Si l'adresse est mal formée, on fallback sur Address::ZERO pour éviter un panic
    let address = Address::from_str(&pos_item.user.address).unwrap_or(Address::ZERO);

    // 2. Extraire et convertir les valeurs numériques de ton type "Number" vers U256
    // Note : Ajuste `.value` ou `.to_u256()` selon la structure réelle de ton type Number
    let borrow_shares = U256::from_str(&pos_item.state.borrow_shares.to_string())
        .unwrap_or(U256::ZERO);
        
    let borrow_assets_usd = f64::from_str(&pos_item.state.borrow_assets_usd.to_string())
        .unwrap_or(0.0);
        
    let collateral_assets = U256::from_str(&pos_item.state.collateral.to_string())
        .unwrap_or(U256::ZERO);

    BorrowPosition {
        market_id,
        address,
        borrow_shares,
        borrow_assets_usd,
        collateral_assets,
        cached_hf: Some(U256::ZERO), // 0 par défaut comme demandé, le calcul se fera après
        onchain_checked: false,
    }
}




 pub async fn fetch_parse_all_market(chain_id: u32) -> anyhow::Result<Vec<MarketParam>> {
    let market_result = fetch_all_market(chain_id).await;    // for testing
    let mut all_markets = Vec::new();
    
    match market_result {
        // 1. On extrait la valeur "result" à l'intérieur du Ok
        Ok(result) => {
            all_markets.extend(result);
            println!("market {}", all_markets.len()); 
        }
        Err(e) => {
            // Si ça plante, on intercepte l'erreur ici !
            println!("❌ Erreur lors de la requête GraphQL : {:?}", e);
            // Affiche la cause exacte (ex: quel champ est 'null')
            println!("🔍 Cause détaillée : {}", e.root_cause());
            // On s'arrête ici en retournant l'erreur au main
            return Err(e);
        }
    }

    let mut all_morpho_markets:Vec<MarketParam> = Vec::new();
    
    for m in &all_markets {
        
        let result = market_item_to_morpho_market(m, chain_id); 
        // filter 
        match result {
            Ok(result) => { 
                
                all_morpho_markets.push(result); 
            }
            Err(err) => {
                continue; 
            }
            
        }
                 
        }
    


    Ok(all_morpho_markets)
}
