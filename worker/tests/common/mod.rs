use std::str::FromStr;

use alloy::{network::{Ethereum, EthereumWallet}, providers::{Provider, ProviderBuilder}, rpc::types::TransactionRequest, signers::local::PrivateKeySigner};
use alloy_primitives::{Address, Bytes, address, U256};
use morpho::utils::WAD; 
use morpho::types::MarketParam; 

pub const USDC:Address = address!("833589fCD6eDb6E08f4c7C32D4f71b54bdA02913");
pub const WETH:Address = address!("0x4200000000000000000000000000000000000006") ; 

const DEPOSIT_SELECTOR: [u8; 4] = [0xd0, 0xe3, 0x0d, 0xb0]; // deposit()
const BALANCE_OF_SELECTOR: [u8; 4] = [0x70, 0xa0, 0x82, 0x31]; // balanceOf(address)
pub const MORPHO: Address = address!("0xBBBBBbbBBb9cC5e90e3b3Af64bdAF62C37EEFFCb"); 

pub mod anvil; 

pub fn weth_usdc_market() -> MarketParam {
        MarketParam {
            // Parse le string hex en FixedBytes<32>
            id: "0x8793cf302b8ffd655ab97bd1c695dbd967807e8367a65cb2f4edaf1380ba1bda"
                .parse()
                .unwrap(),
            
            // Utilise la macro address! pour les adresses
            loan_token: address!("0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913"), // USDC
            collateral_token: address!("0x4200000000000000000000000000000000000006"), // WETH
            oracle: address!("0xFEa2D58cEfCb9fcb597723c6bAE66fFE4193aFE4"),
            
            // Tu peux réutiliser la constante IRM que tu as déjà définie au début !
            irm: address!("0x46415998764C29aB2a25CbeA6254146D50D22687"), 
            
            // Parse le string décimal en U256
            lltv: "860000000000000000".parse().unwrap(), // 86%
            
            // Strings
            loan_token_str: "USDC".to_string(),
            collateral_token_str: "WETH".to_string(),
            
            // Chain ID de Base
            chain_id: 8453, 
            
            // Décimales
            loan_token_decimals: 6,
            collateral_token_decimals: 18,
        }
    }

/// Construit un provider HTTP signé à partir d'une pk, réutilisable partout.
pub async fn provider_addr_from_pk(
    anvil_instance: &anvil::AnvilInstance,
    private_key_hex: &str,
) -> Result<(impl Provider<Ethereum>, Address), Box<dyn std::error::Error>> {
    let signer = PrivateKeySigner::from_str(private_key_hex)?;
    let address = signer.address();
    let wallet = EthereumWallet::from(signer);

    let provider = ProviderBuilder::new()
        .wallet(wallet)
        .connect_http(anvil_instance.endpoint().parse()?);

    Ok((provider, address))
}

/// Encode balanceOf(address) calldata.
pub fn balance_of_calldata(addr: Address) -> Vec<u8> {
    let mut calldata = Vec::with_capacity(4 + 32);
    calldata.extend_from_slice(&BALANCE_OF_SELECTOR);
    calldata.extend_from_slice(&[0u8; 12]);
    calldata.extend_from_slice(addr.as_slice());
    calldata
}



pub async fn approve(provider: impl Provider<Ethereum>, signer_addr: Address, addr_to_approve: Address) -> Result<(), Box<dyn std::error::Error>> {
let approve_selector = eth_core::encode::selector("approve(address,uint256)"); 
    let mut approve_calldata = Vec::with_capacity(4 + 64);
    approve_calldata.extend_from_slice(&approve_selector);
    
    // Padding address spender (Morpho)
    approve_calldata.extend_from_slice(&[0u8; 12]);
    approve_calldata.extend_from_slice(MORPHO.as_slice()); // Assure-toi que MORPHO est dans common.rs
    
    // Montant autorisé (1 WAD = 1e18)
    approve_calldata.extend_from_slice(&U256::from(WAD).to_be_bytes::<32>());

    let approve_tx = TransactionRequest::default()
        .to(addr_to_approve)
        .from(signer_addr)
        .input(approve_calldata.into());

    // Envoi réel de la transaction d'approval
    let pending_approve = provider.send_transaction(approve_tx).await.expect("approve tx failed");
    pending_approve.get_receipt().await.expect("approve receipt failed");
    println!("✓ Approval donné à Morpho pour 1 WAD");
    Ok(())
}
  pub async fn wrap_eth(
    anvil_instance: &anvil::AnvilInstance,
    private_key_hex: &str,
    amount: U256,
) -> Result<(), Box<dyn std::error::Error>> {
    let _signer = PrivateKeySigner::from_str(private_key_hex)?;
    let (provider, signer_addr) = provider_addr_from_pk(anvil_instance, private_key_hex).await?;

    // deposit() -- vraie transaction, pas un call
    let tx = TransactionRequest::default()
        .to(WETH)
        .value(amount)
        .input(Bytes::from(DEPOSIT_SELECTOR.to_vec()).into());

    let pending = provider.send_transaction(tx).await?;
    let receipt = pending.get_receipt().await?;

    if !receipt.status() {
        return Err(format!("tx revertée, status: {:?}", receipt.status()).into());
    }

    // balanceOf(me) -- lecture seule, call est correct ici
    let calldata = balance_of_calldata(signer_addr); 

    let call_tx = TransactionRequest::default()
        .to(WETH)
        .input(Bytes::from(calldata).into());

    let result = Provider::call(&provider, call_tx).await?;
    let balance = U256::from_be_slice(&result);

    println!("✓ WETH balance: {balance} wei");

    if balance != amount {
        return Err(format!("balance WETH attendu {amount}, obtenu {balance}").into());
    }

    Ok(())
}



pub async fn morpho_supply_collateral_weth(anvil_instance: &anvil::AnvilInstance, pk:&str) -> Result<(), Box<dyn std::error::Error>> {
    let sel = eth_core::encode::selector("supplyCollateral((address,address,address,address,uint256),uint256,address,bytes)");
    let (provider, signer_addrr) =  provider_addr_from_pk(anvil_instance, pk).await?; 

    let mut calldata: Vec<u8> = Vec::with_capacity(4+(32*5)+ 32*4); 
    calldata.extend_from_slice(&sel);

    let mparam_weth = weth_usdc_market().to_market_contract_params().to_bytes(); 
    calldata.extend_from_slice(&mparam_weth);
    calldata.extend_from_slice(&U256::from(WAD).to_be_bytes_vec());
    calldata.extend_from_slice(&[0u8; 12]);
    calldata.extend_from_slice(signer_addrr.as_slice());
    calldata.extend_from_slice(&U256::from(256u64).to_be_bytes::<32>());
    // Ensuite on donne la longueur du tableau (ici vide, donc 0)
    calldata.extend_from_slice(&U256::ZERO.to_be_bytes::<32>());

 
    let tx = TransactionRequest::default()
        .to(MORPHO)
        .from(signer_addrr) // Préciser le from est une bonne pratique
        .input(Bytes::from(calldata).into());

    // 5. Envoi réel de la transaction (au lieu d'un simple .call())
    println!("Envoi de la transaction supplyCollateral...");
    let pending_tx = provider.send_transaction(tx).await?;
    let receipt = pending_tx.get_receipt().await?;


    println!("✓ Supply Collateral réussi ! Hash: {:?}", receipt.transaction_hash);
    Ok(())
}


pub async fn morpho_borrow_usdc(anvil_instance: &anvil::AnvilInstance, pk:&str, usdc_amount:U256) -> Result<(), Box<dyn std::error::Error>> {
    let sel = eth_core::encode::selector("borrow((address,address,address,address,uint256),uint256,uint256,address,address)");
    
    let (provider, signer_addrr) =  provider_addr_from_pk(anvil_instance, pk).await?; 
    
    let mut calldata: Vec<u8> = Vec::with_capacity(9*32); 
    calldata.extend_from_slice(&sel);

    let mparam_weth = weth_usdc_market().to_market_contract_params().to_bytes(); 
    calldata.extend_from_slice(&mparam_weth);
    calldata.extend_from_slice(&usdc_amount.to_be_bytes_vec());
    calldata.extend_from_slice(&U256::from(0).to_be_bytes_vec());
    calldata.extend_from_slice(&[0u8; 12]);
    calldata.extend_from_slice(signer_addrr.as_slice());
    calldata.extend_from_slice(&[0u8; 12]);
    calldata.extend_from_slice(signer_addrr.as_slice());


 
    let tx = TransactionRequest::default()
        .to(MORPHO)
        .from(signer_addrr) // Préciser le from est une bonne pratique
        .input(Bytes::from(calldata).into());

    // 5. Envoi réel de la transaction (au lieu d'un simple .call())
    println!("Envoi de la transaction borrow usdc...");
    let pending_tx = provider.send_transaction(tx).await?;
    let receipt = pending_tx.get_receipt().await?;


    println!("✓ Borrow réussi ! Hash: {:?}", receipt.transaction_hash);
    Ok(())
}



/// Force le bytecode d'un "oracle" pour qu'il renvoie toujours `price`,
/// peu importe le calldata reçu. Équivalent du mock utilisé côté Go.
pub async fn set_oracle_price(
    provider: &impl Provider<Ethereum>,
    oracle: Address,
    price: U256,
) -> Result<(), Box<dyn std::error::Error>> {
    // Bytecode : PUSH32 <price> ; PUSH1 0x00 ; MSTORE ; PUSH1 0x20 ; PUSH1 0x00 ; RETURN
    let mut code = Vec::with_capacity(1 + 32 + 8);
    code.push(0x7f); // PUSH32
    code.extend_from_slice(&price.to_be_bytes::<32>());
    code.extend_from_slice(&[0x60, 0x00, 0x52, 0x60, 0x20, 0x60, 0x00, 0xf3]);

    let code_hex = format!("0x{}", hex::encode(&code));

    provider
        .client()
        .request::<_, ()>("anvil_setCode", (oracle.to_string(), code_hex))
        .await?;

    Ok(())
}


/// Appelle `price()` sur l'oracle et retourne la valeur en U256.
pub async fn get_oracle_price(
    provider: &impl Provider<Ethereum>,
    oracle: Address,
) -> Result<U256,  Box<dyn std::error::Error>> {
    let sel = eth_core::encode::selector("price()"); 
    let mut calldata = Vec::with_capacity(4);
    calldata.extend_from_slice(&sel);

    let tx = TransactionRequest::default()
        .to(oracle)
        .input(calldata.into());

    let result = provider
        .call(tx)
        .await?;

    Ok(U256::from_be_slice(&result))
}




