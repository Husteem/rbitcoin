Fixed

- **Core's `wallet_txn_clone.py` passes again.** The functional test
  wallet left an Esplora 404 response open. Python 3.14 then printed a
  finalizer error to the wallet's stderr, and Core's harness fails any
  node that writes to stderr. Esplora answers 404 for a conflicted
  transaction since this release.
