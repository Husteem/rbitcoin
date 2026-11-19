Fixed

- **An IBD `notfound` is no longer block progress.** A peer that withholds a requested block now stalls out even while it keeps sending `notfound`. A `notfound` records only blocks the peer was asked for, and keeps it off each for the full 30 s: padding the message or a later `inv` or `headers` no longer hands the block straight back. The per-peer record expires its oldest answers in O(1) instead of scanning the whole map for every hash.
