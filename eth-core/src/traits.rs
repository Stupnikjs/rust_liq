use alloy::primitives::{Address, Bytes};

use crate::utils::BoxError;



#[async_trait::async_trait]
pub trait CallRaw {
    async fn call_raw(
        &self,
        tier: u8, 
        from: Address,
        to: Address,
        data: Bytes,
    ) -> Result<Bytes, BoxError>;
}