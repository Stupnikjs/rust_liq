use alloy::primitives::{Address, Bytes};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RpcKind {
    Main,
    Secondary,
}


#[async_trait::async_trait]
pub trait CallRaw {
    async fn call_raw(
        &self,
        rpc: RpcKind,
        from: Address,
        to: Address,
        data: Bytes,
    ) -> Result<Bytes, Box<dyn std::error::Error>>;
}