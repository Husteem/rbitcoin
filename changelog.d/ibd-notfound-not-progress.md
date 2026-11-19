Fixed

- **An IBD `notfound` is no longer block progress.** A peer that withholds a requested block now stalls out even while it keeps sending `notfound`. A full per-peer `notfound` record drops its oldest hash in O(1) instead of scanning the whole map for every hash.
