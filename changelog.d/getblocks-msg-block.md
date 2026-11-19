Fixed

- **`getblocks` is answered as Core answers it.** The reply `inv` listed
  `MSG_WITNESS_BLOCK`, a `getdata`-only type, so bitcoinj dropped the
  connection and never synced from a v1 listener. It now lists `MSG_BLOCK`,
  and it stops before `hashStop` instead of announcing it.
