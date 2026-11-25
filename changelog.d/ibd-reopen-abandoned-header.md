Fixed

- **One kept peer's mutated copies no longer retire a block for every
  peer in IBD.** After three mutated copies from a `--connect` or noban
  peer, IBD abandons that header so catch-up can end (#1811). It then
  refused the hash from every peer, even one that announced it later
  with the honest body. A peer that sent none of those copies now puts
  the header back by announcing it, and IBD asks that peer for it. The
  peers that sent the copies are kept off it, and the next mutated copy
  abandons it again.
