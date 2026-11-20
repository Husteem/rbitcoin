Fixed

- **IBD keeps a `--connect` peer that sends a mutated block.** It used to
  disconnect it and dial it again, and with one peer that kept serving the
  block the node never settled. Core never disconnects a manual peer for
  misbehavior; tip-follow already kept one, and IBD already kept a stalling
  one (#1265). The block is still asked for again (#1772).
