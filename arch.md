src/
├── abi/           — encodage manuel des appels (encode.rs, mod.rs)
├── api/           — client GraphQL Morpho + parsing (liquidation, market, pos, queries, types, number)
├── backtest/      — pour alimenter la route /logs en temps reel pour suivre les marchés 
├── cache/         — MarketCache (events, logs, positions, refresh, sort) + server.rs (l'API Axum)
├── connector/     — wrapper alloy (rate_limiter, tx_sender) et call_raw sur RootProvider<Ethereum>, Nonce managing.
├── liquidate/     — construction + encodage des transactions de liquidation
├── morpho/        — types et calls spécifiques au protocole Morpho
├── runner/        — orchestration (api.rs, market_loop.rs, quote.rs, config/), tokio::join sur les handlers independants 
├── swap/          — routing Uniswap V3/PancakeSwap (abi/, quoter.rs, routes.rs)
├── lib.rs, main.rs
tests/
├── common/
└── test_liquidate.rs
data/
├── edges.json, edges.txt, markets.json   — cache local des données de marché