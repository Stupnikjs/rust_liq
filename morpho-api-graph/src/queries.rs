#![allow(dead_code, unused_variables, unused_imports)]

pub fn positions_query(market_id: &str, chain_id: u32, skip: i64) -> String {
    format!(
        r#"{{
            marketPositions(
                first: 1000
                skip: {skip}
                where: {{
                    marketUniqueKey_in: ["{market_id}"]
                    chainId_in: [{chain_id}]
                }}
            ) {{
                items {{
                    user {{ address }}
                    state {{
                        borrowShares borrowAssetsUsd collateral
                    }}
                }}
                pageInfo {{
                    count
                    countTotal
                }}
            }}
        }}"#
    )
}

pub fn markets_query(chain_id: u32, min_borrow_usd:f64) -> String {
    format!(
        r#"{{
            markets(
                orderBy: SupplyAssetsUsd
                orderDirection: Desc
                first: 1000
                where: {{ chainId_in: [{chain_id}], borrowAssetsUsd_gte: {min_borrow_usd} }}
            ) {{
                items {{
                    marketId
                    creationTimestamp
                    oracleAddress
                    lltv
                    irmAddress
                    loanAsset {{ address symbol decimals }}
                    collateralAsset {{ address symbol decimals }}
                    state {{ supplyAssetsUsd borrowAssetsUsd }}
                }}
            }}
        }}"#
    )
}

pub fn liquidations_query(chain_id: u32, skip: i64) -> String {
    format!(
        r#"{{
            transactions(
                first: 1000
                skip: {skip}
                where: {{
                    chainId_in: [{chain_id}]
                    type_in: [MarketLiquidation]
                }}
                orderBy: Timestamp
                orderDirection: Desc
            ) {{
                items {{
                    hash
                    timestamp
                    type
                    data {{
                        ... on MarketLiquidationTransactionData {{
                            seizedAssets
                            repaidAssets
                            seizedAssetsUsd
                            repaidAssetsUsd
                            badDebtAssetsUsd
                            liquidator
                            market {{
                                marketId
                                loanAsset {{ address symbol decimals }}
                                collateralAsset {{ address symbol decimals }}
                            }}
                        }}
                    }}
                }}
                pageInfo {{ count countTotal }}
            }}
        }}"#
    )
}