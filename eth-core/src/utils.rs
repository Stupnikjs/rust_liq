use alloy_primitives::Bytes;
use crate::encode::selector; 


pub fn price_calldata() -> Bytes {
    let sel = selector("price()"); 
    let mut calldata: Vec<u8> = Vec::with_capacity(4);
    calldata.extend_from_slice(&sel);
    calldata.into()
}

use std::error::Error as StdError;

/// Type d'erreur boxé partagé par tout eth-core / connector / worker.
/// `Send + Sync + 'static` est requis pour traverser `tokio::spawn`.
pub type BoxError = Box<dyn StdError + Send + Sync + 'static>;