Fixed

- **Pruned RPC answers match Core's codes.** `getblock` below `pruneheight`
  returns `-1` `Block not available (pruned data)`, and an unscoped
  `getrawtransaction` returns `-5` with Core's no-txindex text. LND can restart
  after more than 288 blocks offline on a pruned backend.
