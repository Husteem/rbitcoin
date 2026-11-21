Fixed

- **A full mempool evicts with one disk sync per pass.** Making room for a
  new transaction used to sync the mempool files once for every chunk it
  dropped, holding up other admits each time. The pass now syncs once,
  after its last chunk and before it returns, so an evicted transaction
  still does not come back after a crash.
