#![allow(dead_code, unused_variables, unused_imports)]
use reqwest::Client;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use std::fmt;

pub mod pos;
pub mod queries;
pub mod types;
pub mod number; 
pub mod market;


const MORPHO_GRAPHQL_URL: &str = "https://api.morpho.org/graphql";


pub use pos::fetch_all_positions;
pub use market::fetch_all_market;


pub struct HttpClient {
    url: String,
    client: Client,
}

// transforme struct en json 
#[derive(Serialize)]
struct QueryBody<'a> {
    query: &'a str,
}


// format de reponse GraphQL
#[derive(Deserialize)]
struct Envelope {
    data: Option<Value>,
    errors: Option<Vec<GraphQLError>>,
}

#[derive(Deserialize, Debug)]
pub struct GraphQLError {
    pub message: String,
}

// pour permettre println!("{}", err)
impl fmt::Display for GraphQLError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "graphql: {}", self.message)
    }
}
// GraphQLError deviens une reele erreur rust
impl std::error::Error for GraphQLError {}

impl HttpClient {
    pub fn new() -> Self {
        Self {
            url: MORPHO_GRAPHQL_URL.to_string(),
            client: Client::new(),
        }
    }


    /*
    le ? est un shortcut pour
    match result {
    Ok(v) => v,
    Err(e) => return Err(e.into())
    }
     */
    pub async fn query<T: DeserializeOwned>(&self, query: &str) -> anyhow::Result<T> {
        let resp = self
            .client
            .post(&self.url)
            .json(&QueryBody { query })
            .send()
            .await?;

        
        // transforme json en envelope 
        let envelope: Envelope = resp.json().await?;

        // err check
        if let Some(mut errors) = envelope.errors {
            if let Some(first) = errors.pop() {
                return Err(first.into());
            }
        }

        let data = envelope
            .data
            .ok_or_else(|| anyhow::anyhow!("empty data field"))?;
        Ok(serde_json::from_value(data)?)
    }
}