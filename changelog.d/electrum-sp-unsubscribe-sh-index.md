Fixed

- **Electrum `blockchain.silentpayments.unsubscribe` works with
  `--sh-index` off.** It answered `scripthash index disabled` although the
  subscribe it ends was served: a silent-payment scan reads the tweak
  index, not the scripthash index.
