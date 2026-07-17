# Liquidateur Morpho

# Les Composants

## Abi
Dans un but pédagogique j'ai préféré l'ABI manuel.

## Api 
query sur l'api morpho et parsing de la reponse json en type adequat

## BackTestDB
Sauvegarde des positions proches de liquidation pour comparer avec les liquidations.
New() crée la table snapshot et spawn la "routine" qui recoit les batch de snapshot 

## Connector
- RootProvider<Ethereum> permet de faire un call_raw() et second_call_raw sur le deuxieme rpc 
- ws est un Arc<RootProvider<Ethereum>> pour subscribe et listen pour les logs de morphoblue 
- rate limiter pour ne pas depasser le free tier alchemy 
- tx_sender qui gere le nonce managing et le gas pour envoyer des tx signée 






## Liquidator

## MarketCache

## Runner
- Déploiement des market loops
- Fetch API
- Quote

## Swap 


## Market Loop 
estimation du temps a sleep avant le nouveau refresh. 
=> dépend de la distance avec la prochaine liquidation. 
=> 

