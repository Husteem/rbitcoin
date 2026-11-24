Fixed

- **Pruned RPC answers match Core's codes.** `getblock`, `getblockstats`, and
  a blockhash-scoped `getrawtransaction` below `pruneheight` return `-1`; an
  unscoped `getrawtransaction` returns `-5` with Core's no-txindex text. LND
  can restart after more than 288 blocks offline on a pruned backend.
