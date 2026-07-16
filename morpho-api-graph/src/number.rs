#![allow(dead_code, unused_variables, unused_imports)]
// api/number.rs
use serde::{Deserialize, Deserializer, Serialize, de::Error};
use std::fmt;

/// Équivalent de json.Number : garde le texte brut, accepte
/// aussi bien un nombre JSON qu'une string JSON.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Number(pub String);

impl Number {
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub fn parse_f64(&self) -> Result<f64, std::num::ParseFloatError> {
        self.0.parse()
    }
    pub fn parse_u128(&self) -> Result<u128, std::num::ParseIntError> {
        self.0.parse()
    }
}

impl fmt::Display for Number {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl<'de> Deserialize<'de> for Number {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        match serde_json::Value::deserialize(d)? {
            serde_json::Value::String(s) => Ok(Number(s)),
            serde_json::Value::Number(n) => Ok(Number(n.to_string())),
            other => Err(D::Error::custom(format!("expected number or string, got {other}"))),
        }
    }
}