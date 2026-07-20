use alloy::primitives::{Address, Bytes};

#[async_trait::async_trait]
pub trait CallRaw {
    async fn call_raw(
        &self,
        from:Address,
        to: Address,
        data: Bytes,
    ) -> Result<Bytes, Box<dyn std::error::Error>>;
}