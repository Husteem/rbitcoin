Fixed

- **Power loss no longer leaves the chain naming lost transactions.** The
  connect barrier now device-syncs every archive file a block commit names
  (outputs, inputs and witness, spend slots, both locator families, txids,
  per-tx stats, and parent edges) before it writes the confirmed tip. A
  power cut could leave the tip, a strong bit, or a block's transaction
  range pointing at bytes that never reached the disk. The node then
  refused to start, rejected the next valid block, or wedged on one block
  until the store was wiped. Those syncs go to the configured bulk-IO
  backend (io_uring, the IO pool, or IOCP) together, so they overlap
  instead of queuing.
- **An unpublished archive tail is cut on open.** A body header that fell
  short of its locator refused startup ("body header short of locator"),
  and a header that landed without its rows refused ("overflow missing")
  or kept a transaction that could not be read. Open now keeps exactly the
  transactions a durable record names (a block range, a spend checkpoint,
  or an address-index mark) and cuts the rest without reading it, so a
  torn row that happens to decode is not kept. The cut blocks download
  again.
  A short `input.body` header cuts the input locator the same way, and an
  input locator past that prefix is cut. A datadir torn below a committed
  block (by an older binary, or a disk that lost synced writes) refuses to
  start and asks for a reindex instead of cutting below that block:
  cutting it would let stale spends and address history attach to a later
  transaction. A failed archive write or sync now latches until restart,
  so a retried fsync cannot commit over pages the kernel dropped and no
  checkpoint or mark can name a transaction whose header write failed.
  The node stops with an error instead of failing every later block; a
  full disk now needs a restart once space is freed. A spend checkpoint
  syncs the archive before its spend delta commit can name a spender.
- **An interrupted `tx.head` rebuild rebuilds again.** Segment files with
  no `tx.head/meta` refused startup as a legacy layout and needed a manual
  delete. Open now wipes that directory and rebuilds it from the archive.
- **Small state files are published whole.** Store `meta`, `store.secret`,
  the mempool `meta`, and the unbroadcast list are written to a synced
  temp file and renamed, with the directory synced. `store.secret` is
  created owner-only.
- **Torn fee/size stats read as missing.** A power cut during a
  `getblockstats` stamp could leave a stats cell naming an overflow record
  that never landed. Every later `getblockstats` for that block then
  failed with "txstat overflow missing". The row now reads as unstamped,
  and the next call stamps it again.
- **A torn address index rebuilds itself.** When the scripthash
  write-behind reads a torn record after a power cut, it marks the index
  for a full rebuild on the next start instead of failing every few
  blocks. Before, `RBITCOIN_SH_FORCE_REBUILD` was the only remedy.
