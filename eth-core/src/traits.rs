use alloy::primitives::{Address, Bytes};

use crate::utils::BoxError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RpcKind {
    Main,
    Secondary,
}


#[async_trait::async_trait]
pub trait CallRaw {
    async fn call_raw(
        &self,
        top_tier: bool, 
        from: Address,
        to: Address,
        data: Bytes,
    ) -> Result<Bytes, BoxError>;
}