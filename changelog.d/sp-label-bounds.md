Fixed

- **Silent-payment labels are read as BIP-352 `u32` values and capped as
  they are read.** `blockchain.silentpayments.subscribe` folded a label of
  2^32 or more into a smaller one (2^32 became label 0), and it
  deduplicated the whole label list before refusing more than ten, so one
  long list cost quadratic CPU. A label past `u32` is now refused, and the
  eleventh distinct label stops the read.
