Fixed
- **Class C and derived writes survive a torn in-place page.** A
  `header_txs` append rewrote the block holding the previous tip's committed
  range, `header.head` rewrote a 3 KiB chunk beside committed slots, and
  `txstat.body` stamped cells in place. A power cut could tear those pages, and
  open accepted a missing `header_txs` range as `Ok` or served a
  well-formed wrong stats or filter value. Those writes now put a synced
  `*.ahead` image of the intended pages first; open restores it. A torn
  strong flush is repaired from the confirmed fence instead. `txstat.blk`
  and `txstat.ovf` are unchanged ([#2071],
  [#2072], [#2073], [#2075]).
- **A later write keeps outstanding `*.ahead` page images.** Replacing the
  sidecar merges every image the new write does not cover; new images win on
  overlap. `HashHead`, `ScriptHashHead`, in-place `txstat` stamps, and
  `header_txs` share that merge ([#2108]).
- **txstat appends leave the Class A tail guard in charge of new cells.** An
  append-only batch does not copy the new tail into `txstat.body.ahead`. It
  drops leftover images that overlap the append and keeps pages it does not
  touch. Restore will not shrink a durable length past the sidecar or replay
  bytes the append already published. The in-place `getblockstats` stamp
  still journals its page set.
- **Scripthash free-list and allocator state are ordered and checked.** The
  unlink batch syncs freed links before the allocator header that names
  them, the header's `bump` and free heads must be in range, a popped link
  must be in range, and a slab's decoded count must match its head's `used`.
  A torn allocator or slab now fails into the degraded/rebuild path instead
  of handing a live slab to another key ([#2074]).
- **A served block filter is checked against its index hash.** A torn body
  no longer returns filter bytes that do not match the stored
  `filter_hash`.
- A torn block-filter parent rebuild is still deferred ([#2109]).

[#2071]: https://github.com/rbitcoin-org/rbitcoin/issues/2071
[#2072]: https://github.com/rbitcoin-org/rbitcoin/issues/2072
[#2073]: https://github.com/rbitcoin-org/rbitcoin/issues/2073
[#2074]: https://github.com/rbitcoin-org/rbitcoin/issues/2074
[#2075]: https://github.com/rbitcoin-org/rbitcoin/issues/2075
[#2108]: https://github.com/rbitcoin-org/rbitcoin/issues/2108
[#2109]: https://github.com/rbitcoin-org/rbitcoin/issues/2109
