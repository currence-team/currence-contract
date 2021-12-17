## build/deploy
```
yarn near login
export account_id=whatever.testnet
```

```
cargo build --target wasm32-unknown-unknown --release
yarn near dev-deploy --wasmFile target/wasm32-unknown-unknown/release/prediction_market.wasm
# get the dev contract ID

export dev_id=whatever

yarn near call ${dev_id} create_market --accountId ${account_id} '{"args": {"title": "test title", "description": "test description", "collateral_token": "test collateral token", "collateral_decimals": 1, "end_time": 1, "resolution_time": 1, "outcomes": [], "trade_fee_bps": 0}}'
```
