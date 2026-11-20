Fixed

- **Tip-follow reads a `block` message with bytes after its last
  transaction,** as Core does. The message was dropped as unknown, so a
  peer that sent one never delivered that block. `submitblock` and the IBD
  body path already ignored those bytes (#1773).
