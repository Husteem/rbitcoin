Fixed

- **A panicked `tx.head` insert keeps the batch.** `ibd-confirm-head` leaves that batch parked until the insert returns `Ok`. The write puts it back on the pending list when the worker panics.
