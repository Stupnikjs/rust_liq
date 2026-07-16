use alloy::primitives::{keccak256, Address, Bytes, U256, B256};

// ── selector ─────────────────────────────────────────────────────
pub fn selector(signature: &str) -> [u8; 4] {
    let hash = keccak256(signature.as_bytes());
    let mut sel = [0u8; 4];
    sel.copy_from_slice(&hash[..4]);
    sel
}

// ── calldata builder ─────────────────────────────────────────────
pub fn encode_calldata(sel: [u8; 4], args: &[u8]) -> Bytes {
    let mut data = Vec::with_capacity(4 + args.len());
    data.extend_from_slice(&sel);
    data.extend_from_slice(args);
    data.into()
}

pub fn encode_args(parts: &[[u8; 32]]) -> Vec<u8> {
    let mut out = Vec::with_capacity(parts.len() * 32);
    for p in parts {
        out.extend_from_slice(p);
    }
    out
}

// ── encode args ───────────────────────────────────────────────────
#[inline(always)]
pub fn encode_address(addr: Address) -> [u8; 32] {
    let mut buf = [0u8; 32];
    buf[12..32].copy_from_slice(addr.as_slice());
    buf
}

#[inline(always)]
pub fn encode_uint256(val: U256) -> [u8; 32] {
    val.to_be_bytes::<32>()
}

#[inline(always)]
pub fn encode_bool(val: bool) -> [u8; 32] {
    let mut buf = [0u8; 32];
    buf[31] = val as u8;
    buf
}

#[inline(always)]
pub fn encode_bytes32(val: B256) -> [u8; 32] {
    val.0
}

// ── decode ─────────────────────────────────────────────────────────
#[inline(always)]
pub fn decode_uint(data: &[u8]) -> U256 {
    U256::from_be_slice(&data[0..32])
}

#[inline(always)]
pub fn decode_address(data: &[u8]) -> Address {
    Address::from_slice(&data[12..32])
}

#[inline(always)]
pub fn decode_bool(data: &[u8]) -> bool {
    data[31] != 0
}

#[inline(always)]
pub fn decode_bytes32(data: &[u8]) -> B256 {
    B256::from_slice(&data[0..32])
}

/// decode N valeurs de 32 bytes consécutives (le cas le plus courant — Morpho market(), etc.)
pub fn decode_uint_n(data: &[u8], n: usize) -> Vec<U256> {
    (0..n)
        .map(|i| U256::from_be_slice(&data[i * 32..(i + 1) * 32]))
        .collect()
}

// string nécessite de lire l'offset dynamique — seul cas non-trivial
pub fn decode_string(data: &[u8]) -> Result<String, Box<dyn std::error::Error>> {
    let offset = decode_uint(&data[0..32]).to::<usize>();
    let len    = decode_uint(&data[offset..offset + 32]).to::<usize>();
    let bytes  = &data[offset + 32..offset + 32 + len];
    Ok(String::from_utf8(bytes.to_vec())?)
}