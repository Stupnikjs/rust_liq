#!/usr/bin/env bash

CONTRACT="0x1bb6b60c72bbc80d77f34919c724d2255d24a874"
RPC="https://mainnet.base.org"  # ou ton endpoint dRPC/Ankr premium

declare -A TOKENS=(
  ["USDC"]="0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913"
  ["WETH"]="0x4200000000000000000000000000000000000006"
  ["cbBTC"]="0xcbB7C0000aB88B473b1f5aFd9ef808440eed33Bf"
  ["SOL"]="0x311935Cd80B76769bF2ecC9D8Ab7635b2139cf82"
  ["wstETH"]="0xc1CBa3fCEa344f92D9239c08C0568f6F2F0ee452"
)

for name in "${!TOKENS[@]}"; do
  token="${TOKENS[$name]}"
  decimals=$(cast call "$token" "decimals()(uint8)" --rpc-url "$RPC")
  raw=$(cast call "$token" "balanceOf(address)(uint256)" "$CONTRACT" --rpc-url "$RPC")
  human=$(cast --to-unit "$raw" "$decimals")
  echo "$name: $raw (raw) / $human"
done

# balance native ETH
eth_bal=$(cast balance "$CONTRACT" --rpc-url "$RPC")
echo "ETH: $eth_bal"




cast call 0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913 "balanceOf(address)(uint256)" 0x1bb6b60c72bbc80d77f34919c724d2255d24a874 --rpc-url https://mainnet.base.org
cast call 0x4200000000000000000000000000000000000006 "balanceOf(address)(uint256)" 0x1bb6b60c72bbc80d77f34919c724d2255d24a874 --rpc-url https://mainnet.base.org
cast call 0xcbB7C0000aB88B473b1f5aFd9ef808440eed33Bf "balanceOf(address)(uint256)" 0x1bb6b60c72bbc80d77f34919c724d2255d24a874 --rpc-url https://mainnet.base.org
cast call 0x311935Cd80B76769bF2ecC9D8Ab7635b2139cf82 "balanceOf(address)(uint256)" 0x1bb6b60c72bbc80d77f34919c724d2255d24a874 --rpc-url https://mainnet.base.org
cast call 0xc1CBa3fCEa344f92D9239c08C0568f6F2F0ee452 "balanceOf(address)(uint256)" 0x1bb6b60c72bbc80d77f34919c724d2255d24a874 --rpc-url https://mainnet.base.org