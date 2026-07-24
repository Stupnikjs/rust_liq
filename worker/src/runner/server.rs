use alloy_primitives::FixedBytes;
use axum::{extract::{State, Path}, routing::get, Json, Router};
use std::{str::FromStr, sync::Arc};
use crate::{backtest::{BacktestSnapshot, BacktestStore}, cache::MarketCache};
use crate::cache::logs::{snap_to_market_log, id_to_market_log, MarketLog};

#[derive(Clone)]
struct ServerConsumer {
    cache: Arc<MarketCache>,
    store: Arc<BacktestStore>,
}

pub fn build_router(cache: Arc<MarketCache>, store: Arc<BacktestStore>) -> Router {
    let consumer = ServerConsumer { cache, store };
    Router::new()
        .route("/logs", get(all_logs))
    //  .route("/logs/{id}", get(one_log))
    //  .route("/snap/{id}", get(snap_by_market_id))
        .with_state(consumer)
}

async fn all_logs(State(consumer): State<ServerConsumer>) -> Json<Vec<MarketLog>> {
    let logs: Vec<MarketLog> = consumer.cache
        .ids()
        .into_iter()
        .map(|id| id_to_market_log(&consumer.cache, id))
        .collect();

    Json(logs)
}


/*
async fn one_log(
    State(consumer): State<ServerConsumer>,
    Path(id): Path<String>,
) -> Result<Json<MarketLog>, axum::http::StatusCode> {
    let fixed_id = FixedBytes::<32>::from_str(&id)
        .map_err(|_| axum::http::StatusCode::BAD_REQUEST)?;
    consumer.cache
        .all_snapshots()
        .values()
        .find(|s| s.params.id.eq(&fixed_id))
        .map(snap_to_market_log)
        .map(Json)
        .ok_or(axum::http::StatusCode::NOT_FOUND)
}




async fn snap_by_market_id(
    State(consumer): State<ServerConsumer>,
    Path(id): Path<String>,
) -> Result<Json<Vec<BacktestSnapshot>>, axum::http::StatusCode> {
    let snapshots = consumer.store
        .get_snapshots(id, 4000)
        .await
        .map_err(|e| {
            eprintln!("get_snapshots failed: {e}");
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        })?;

    if snapshots.is_empty() {
        return Err(axum::http::StatusCode::NOT_FOUND);
    }

    Ok(Json(snapshots))
}

    */