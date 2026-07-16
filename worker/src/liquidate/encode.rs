// src/abi/liquidate.rs
use alloy::primitives::{Address, Bytes, U256};
use eth_core::encode::{encode_address, encode_uint256, selector};
use crate::morpho::types::MarketParam; 
use crate::swap::SwapStep;


// encode un tuple fixe MarketParams → 5 slots de 32 bytes
fn encode_market_params(mp: &MarketParam) -> Vec<u8> {
    let mut out = Vec::with_capacity(5 * 32);
    out.extend_from_slice(&encode_address(mp.loan_token));
    out.extend_from_slice(&encode_address(mp.collateral_token));
    out.extend_from_slice(&encode_address(mp.oracle));
    out.extend_from_slice(&encode_address(mp.irm));
    out.extend_from_slice(&encode_uint256(mp.lltv));
    out
}

// encode un SwapStep — contient `bytes data` donc dynamique
// retourne (head: [u8;32] offset, tail: Vec<u8> contenu)
fn encode_swap_step(step: &SwapStep, base_offset: usize) -> (Vec<u8>, Vec<u8>) {
    // tail = contenu du tuple SwapStep
    // layout: target(32) | data_offset(32) | token_in(32) | token_out(32) | amount_in_offset(32) | data_len(32) | data_padded
    let data_offset: usize = 5 * 32; // offset de `data` dans le tuple = 5 slots fixes avant lui
    
    let data_len = step.data.len();
    let padded_len = (data_len + 31) / 32 * 32;

    let mut tail = Vec::new();
    tail.extend_from_slice(&encode_address(step.target));
    tail.extend_from_slice(&encode_uint256(U256::from(data_offset)));
    tail.extend_from_slice(&encode_address(step.token_in));
    tail.extend_from_slice(&encode_address(step.token_out));
    tail.extend_from_slice(&encode_uint256(step.amount_in_offset));
    // bytes data
    tail.extend_from_slice(&encode_uint256(U256::from(data_len)));
    tail.extend_from_slice(&step.data);
    tail.resize(tail.len() + (padded_len - data_len), 0u8);

    // head = offset absolu de ce tuple dans le array tail
    let mut head = [0u8; 32];
    head[..32].copy_from_slice(&encode_uint256(U256::from(base_offset)));

    (head.to_vec(), tail)
}

// encode SwapStep[] complet
fn encode_steps(steps: Vec<SwapStep>) -> Vec<u8> {
    let n = steps.len();
    // layout array: len(32) | offset_0(32) | ... | offset_n-1(32) | tuple_0 | ... | tuple_n-1
    let mut heads: Vec<u8> = Vec::new();
    let mut tails: Vec<u8> = Vec::new();

    // les offsets sont relatifs au début du contenu du array (après le slot len)
    // head area = n * 32 bytes
    let head_area = n * 32;

    for step in steps {
        let base_offset = head_area + tails.len();
        let (head, tail) = encode_swap_step(&step, base_offset);
        heads.extend_from_slice(&head);
        tails.extend_from_slice(&tail);
    }

    let mut out = Vec::new();
    out.extend_from_slice(&encode_uint256(U256::from(n))); // array length
    out.extend_from_slice(&heads);
    out.extend_from_slice(&tails);
    out
}

pub fn encode_liquidate(
    market_param: &MarketParam,
    borrower: Address,
    seized_assets: U256,
    repaid_shares: U256,
    steps: Vec<SwapStep>,
    min_out: U256,
) -> Bytes {
    // selector: liquidate((address,address,address,address,uint256),address,uint256,uint256,(address,bytes,address,address,uint256)[],uint256)
    let sel = selector(
        "liquidate((address,address,address,address,uint256),address,uint256,uint256,(address,bytes,address,address,uint256)[],uint256)"
    );

    // args layout (6 params):
    // [0]  MarketParams tuple  → fixe, inline (5*32 = 160 bytes)
    // [1]  borrower            → fixe 32
    // [2]  seizedAssets        → fixe 32
    // [3]  repaidShares        → fixe 32
    // [4]  steps[]             → dynamique → slot = offset
    // [5]  minOut              → fixe 32

    // offset de steps[] = taille de la head area = 6 * 32 + 160 - 32 = 5*32 + 160
    // head area: mp(160) + borrower(32) + seized(32) + repaid(32) + steps_offset(32) + minout(32) = 320
    let steps_offset: usize = 320; // offset depuis début des args

    let mp_encoded = encode_market_params(market_param);
    let steps_encoded = encode_steps(steps);

    let mut args = Vec::new();
    args.extend_from_slice(&mp_encoded);                              // 160 bytes
    args.extend_from_slice(&encode_address(borrower));               // 32
    args.extend_from_slice(&encode_uint256(seized_assets));          // 32
    args.extend_from_slice(&encode_uint256(repaid_shares));          // 32
    args.extend_from_slice(&encode_uint256(U256::from(steps_offset))); // 32 — offset vers steps[]
    args.extend_from_slice(&encode_uint256(min_out));                // 32
    args.extend_from_slice(&steps_encoded);                          // tail dynamique

    let mut out = Vec::with_capacity(4 + args.len());
    out.extend_from_slice(&sel);
    out.extend_from_slice(&args);
    out.into()
}