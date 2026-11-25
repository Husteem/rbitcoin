Security

- **REST can no longer take every store-read permit.** With `--rest`,
  each REST request held a permit from the store-read budget that
  Electrum, Esplora and RPC share while it built its reply, so a few large
  requests such as `/rest/mempool/contents.json` could stall every other
  store read. REST now takes its own share first, at most half of that
  budget.
