use std::time::{Duration, Instant};
use alloy::primitives::{Address, U256, Bytes};
use crate::swap::PoolEdge;
use connector::Connector;
use eth_core::encode::{encode_address, encode_uint256,selector}; 




// exactInputSingle(address,address,uint24,address,uint256,uint256,uint160)
// amountIn placeholder = 0, patché on-chain via amountInOffset

pub fn encode_quote_single_exact_input(
    token_in: Address,
    token_out: Address,
    fee: u32,
    amount_in: U256,
) -> Bytes {
  
    let sel = selector("quoteExactInputSingle((address,address,uint256,uint24,uint160))");
    let mut args = Vec::with_capacity(5 * 32);
    
    // offset vers le tuple — obligatoire pour un argument tuple en ABI
    args.extend_from_slice(&encode_address(token_in));
    args.extend_from_slice(&encode_address(token_out));
    args.extend_from_slice(&encode_uint256(amount_in));
    args.extend_from_slice(&encode_uint256(U256::from(fee)));
    args.extend_from_slice(&encode_uint256(U256::ZERO)); // sqrtPriceLimitX96

    let mut out = Vec::with_capacity(4 + args.len());
    out.extend_from_slice(&sel);
    out.extend_from_slice(&args);
    out.into()
}



pub fn encode_exact_input_single_uni(
    token_in: Address,
    token_out: Address,
    fee: u32,
    recipient: Address,
) -> Bytes {
    let sel = selector("exactInputSingle((address,address,uint24,address,uint256,uint256,uint160))");
    let mut args = Vec::with_capacity(7 * 32);
    args.extend_from_slice(&encode_address(token_in));
    args.extend_from_slice(&encode_address(token_out));
    args.extend_from_slice(&encode_uint256(U256::from(fee)));
    args.extend_from_slice(&encode_address(recipient));
    args.extend_from_slice(&encode_uint256(U256::ZERO)); // amountIn placeholder
    args.extend_from_slice(&encode_uint256(U256::ZERO)); // amountOutMinimum
    args.extend_from_slice(&encode_uint256(U256::ZERO)); // sqrtPriceLimitX96

    let mut out = Vec::with_capacity(4 + args.len());
    out.extend_from_slice(&sel);
    out.extend_from_slice(&args);
    out.into()
}
