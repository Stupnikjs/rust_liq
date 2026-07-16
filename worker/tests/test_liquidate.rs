// tests/test_anvil.rs
use std::env::var;
use alloy::signers::local::PrivateKeySigner;

use alloy_primitives::{U256, Uint};
use common::anvil::AnvilInstance; // <- IMPORTANT : on précise que ça vient du mod anvil
use alloy::providers::{Provider, ProviderBuilder};
use alloy::{
    hex,
    primitives::{address},
    rpc::types::TransactionRequest,
};
use alloy::primitives::utils::parse_ether;
use worker::cache::positions::BorrowPosition;
use worker::liquidate::build::{Liquidable, to_liquidation_calldata};
use worker::morpho::utils::{WAD}; 
mod common; 
use common::{wrap_eth,provider_addr_from_pk, morpho_supply_collateral_weth, morpho_borrow_usdc};
use worker::swap;

use crate::common::{USDC, WETH, approve, balance_of_calldata, get_oracle_price, set_oracle_price, weth_usdc_market};
 



fn get_rpc_url() -> String {
    dotenvy::dotenv().ok();
    String::from(var("ALCHEMY_BASE_HTTP").expect("ALCHEMY_BASE_HTTP not set"))
}

#[tokio::test]
async fn test_anvil_basic() {
    let anvil = AnvilInstance::fork(&get_rpc_url(), None).unwrap();

    println!("{}", anvil.endpoint()); 
    let provider = ProviderBuilder::new()
        .connect_http(anvil.endpoint().parse().unwrap());

    let block = provider.get_block_number().await.unwrap();
    println!("✓ Block number: {}", block);

    let chain_id = provider.get_chain_id().await.unwrap();
    println!("✓ Chain ID: {}", chain_id);
    assert_eq!(chain_id, 8453);

}

#[tokio::test]
async fn test_anvil_fork_at_block() {
    let target_block = 19_500_000u64;
    let anvil = AnvilInstance::fork(&get_rpc_url(), Some(target_block)).unwrap();

    let provider = ProviderBuilder::new()
        .connect_http(anvil.endpoint().parse().expect("invalid endpoint"));

    let block = provider.get_block_number().await.unwrap();
    println!("Forked at block: {}", block);
    assert_eq!(block, target_block);
}

#[tokio::test]
async fn test_anvil_endpoints() {
    let anvil = AnvilInstance::fork(&get_rpc_url(), None).unwrap();

    println!("HTTP: {}", anvil.endpoint());
    println!("WS:   {}", anvil.ws_endpoint());

    assert!(anvil.endpoint().starts_with("http://127.0.0.1:"));
    assert!(anvil.ws_endpoint().starts_with("ws://127.0.0.1:"));
}






#[tokio::test]
async fn test_usdc_balance_raw() {
    let market = weth_usdc_market(); 
    let anvil = AnvilInstance::fork(&get_rpc_url(), None).unwrap();

    let provider = ProviderBuilder::new()
        .connect_http(anvil.endpoint().parse().unwrap());

    let owner = address!("4200000000000000000000000000000000000006");

    // selector balanceOf(address)
    let mut calldata = Vec::with_capacity(4 + 32);

    calldata.extend_from_slice(&hex!("70a08231"));

    // ABI padding
    calldata.extend_from_slice(&[0u8; 12]);
    calldata.extend_from_slice(owner.as_slice());

    let tx = TransactionRequest::default().to(market.loan_token).input(calldata.into());
    let result = provider.call(tx).await.unwrap(); 
    println!("0x{}", hex::encode(&result));

    // balance est un uint256 big-endian
    let balance = alloy::primitives::U256::from_be_slice(&result);

    println!("Balance = {}", balance);
    println!("USDC = {}", balance / alloy::primitives::U256::from(1_000_000u64));
}


#[tokio::test]
async fn test_wrap_eth() {
    let anvil = AnvilInstance::fork(&get_rpc_url(), None).unwrap();

    let provider = ProviderBuilder::new()
        .connect_http(anvil.endpoint().parse().unwrap());
    let pk ="0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";  
    let signer:PrivateKeySigner =  pk.parse().expect("err parsing pk");   
    use alloy::primitives::utils::parse_ether;
    let amount = parse_ether("10").unwrap();
    wrap_eth(&anvil, pk, amount).await.expect("wrapping ether failed"); 
    // selector balanceOf(address)
    let mut calldata = Vec::with_capacity(4 + 32);
    calldata.extend_from_slice(&hex!("70a08231"));

    // ABI padding
    calldata.extend_from_slice(&[0u8; 12]);
    calldata.extend_from_slice(signer.address().as_slice());

    let tx = TransactionRequest::default().to(common::WETH).input(calldata.into());
    let result = provider.call(tx).await.unwrap(); 
    println!("0x{}", hex::encode(&result));

    // balance est un uint256 big-endian
    let balance = alloy::primitives::U256::from_be_slice(&result);
    let expected = parse_ether("10").unwrap();
    assert_eq!(balance, expected, "balance WETH incorrecte après wrap");
}






#[tokio::test]
async fn test_supply_collateral_borrow() {
    let anvil = AnvilInstance::fork(&get_rpc_url(), None).unwrap();
    let pk = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
    let (provider, signer_addr) = provider_addr_from_pk(&anvil, pk).await.expect("error getting provider addr"); 
    let market_param = weth_usdc_market(); 
    let wrap_amount = parse_ether("2").unwrap();
    wrap_eth(&anvil, pk, wrap_amount).await.expect("wrapping ether failed"); 
    approve(&provider, signer_addr, common::WETH).await.expect("approving WETH failed");
    // ---------------------------------------------------------
    // ÉTAPE 3 : Récupération du solde AVANT le supply
    // ---------------------------------------------------------
    let bal_calldata = balance_of_calldata(signer_addr); 
    let bal_tx = TransactionRequest::default().to(common::WETH).input(bal_calldata.clone().into());
    let result_before = provider.call(bal_tx).await.unwrap();
    let balance_before = U256::from_be_slice(&result_before);
    println!("Balance WETH avant supply: {balance_before}");
    
    approve(&provider, signer_addr, common::USDC).await.expect("approving USDC failed");
    morpho_supply_collateral_weth(&anvil, pk).await.expect("supply collateral failed");

    
    // ---------------------------------------------------------
    // ÉTAPE 5 : Récupération du solde APRÈS et Assertion
    // ---------------------------------------------------------
    let bal_tx_after = TransactionRequest::default().to(common::WETH).input(bal_calldata.into());
    let result_after = provider.call(bal_tx_after).await.unwrap();
    let balance_after = U256::from_be_slice(&result_after);
    println!("Balance WETH après supply: {balance_after}");

    // Le solde doit avoir exactement baissé de 1 WAD (1 WETH)
    assert_eq!(
        balance_before - balance_after, 
        U256::from(WAD), 
        "Le solde WETH n'a pas baissé du bon montant"
    );


    let weth_usdc_price = get_oracle_price(&provider, market_param.oracle).await.expect("error price() call");
    println!("price {}", weth_usdc_price); 
    let max_borrow_usdc_amount = weth_usdc_price
    .checked_mul(Uint::from(95)).and_then(|v| v.checked_div(Uint::from(100)))
    .and_then(|v| v.checked_mul(WAD))
    .and_then(|v| v.checked_mul(market_param.lltv))
    .and_then(|v| v.checked_div(Uint::from(10).pow(Uint::from(36))))
    .and_then(|v| v.checked_div(Uint::from(10).pow(Uint::from(18)))) 
    .expect("overflow dans le calcul max_borrow");

    morpho_borrow_usdc(&anvil, pk, U256::from(max_borrow_usdc_amount)).await.expect("failed borrow usdc");

    let balance_usdc_calldata = balance_of_calldata(signer_addr); 
    let bal_tx = TransactionRequest::default().to(common::USDC).input(balance_usdc_calldata.clone().into());
    let result = provider.call(bal_tx).await.unwrap();
    let balance_after = U256::from_be_slice(&result);
    println!("Balance USDC après supply: {balance_after}"); 
    println!("Max borrow usdc amount après supply: {max_borrow_usdc_amount}"); 
    assert_eq!(balance_after, max_borrow_usdc_amount, "testing amount of USDC borrowed"); 

    
    // crash oracle price 
    let new_price = weth_usdc_price.checked_mul(U256::from(80)).and_then(|v| v.checked_div(U256::from(100))).expect("error seting new price"); 
    println!("new price {}", new_price); 
    let _  = set_oracle_price(&provider, market_param.oracle,new_price).await; 
    let new_called_weth_usdc_price = get_oracle_price(&provider, market_param.oracle).await.expect("error price() call");
    assert_eq!(new_price, new_called_weth_usdc_price);

    let wallet_pk = var("MY_SAFE_PK").expect("ERROR GETTING PK");
    let (wallet_provider, _wallet_addr) = common::provider_addr_from_pk(&anvil, wallet_pk.as_str()).await.expect("error getting wallet provider"); 
    let pos = BorrowPosition{
        market_id: market_param.id, 
        borrow_shares: U256::ZERO,
        borrow_assets_usd: 0.0, 
        collateral_assets: WAD,
        address: signer_addr, 
        cached_hf: None,
        onchain_checked: false,
    }; 


    let liquidable = &Liquidable { borrower: pos.address, seize_assets: WAD, repay_shares: U256::ZERO };
    
    let edge = swap::PoolEdge{
        token_in: WETH,
        token_out: USDC,
        fee: 500,
        wc_amount_in: Uint::from(0), 
        wc_amount_out:  Uint::from(0),
        wc_slippage:5.0,
        dex_name: String::from("UNIV3"),
        amount_in_offset:164, 
        calibrated_at: 0,
        price_at_quote: Uint::from(0),
        router: address!("2626664c2603336E57B271c5C0b26F421741e481"),
    }; 
    let mut route: Vec<swap::PoolEdge> = Vec::with_capacity(1); 
    route.push(edge);
    let liquidator_addr = address!("0x1BB6b60C72bBc80D77f34919C724D2255D24A874"); 
    let liq_calldata = to_liquidation_calldata(liquidator_addr, liquidable, &market_param, &route).expect("error getting liquidatio call data "); 
    let liquidation_tx = TransactionRequest::default().to(liquidator_addr).input(liq_calldata.clone().into());
    let pending = wallet_provider.send_transaction(liquidation_tx).await.expect("tx send failed");
    let receipt = pending.get_receipt().await.expect("failed to get receipt");
    assert!(receipt.status(), "liquidation tx reverted");
  
  
     
    let new_balance_weth_calldata = balance_of_calldata(liquidator_addr); 
    let new_bal_tx = TransactionRequest::default().to(common::WETH).input(new_balance_weth_calldata.clone().into());
    let new_result = provider.call(new_bal_tx).await.unwrap();
    let new_balance_after = U256::from_be_slice(&new_result);
     println!("Balance wallet WETH après supply: {new_balance_after}"); 

    let new_balance_usdc_calldata = balance_of_calldata(liquidator_addr); 
    let new_bal_tx = TransactionRequest::default().to(common::USDC).input(new_balance_usdc_calldata.clone().into());
    let new_result = provider.call(new_bal_tx).await.unwrap();
    let new_balance_after = U256::from_be_slice(&new_result);

    

    println!("Balance wallet USDC après supply: {new_balance_after}"); 
    
}