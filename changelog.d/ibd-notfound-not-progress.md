Fixed

- **An IBD `notfound` is no longer block progress.** A peer that still owes a requested block now stalls out even while it keeps sending `notfound`. A `notfound` counts only for blocks the peer was asked for, and keeps the peer off each for 30 s unless it announces that block: padding the message or announcing other blocks no longer hands the block straight back. When the per-peer record is full, the block stays owed. The record expires its oldest answers in O(1) instead of scanning the whole map for every hash.
