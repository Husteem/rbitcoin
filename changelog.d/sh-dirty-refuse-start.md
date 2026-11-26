Changed

- **A start after an unclean stop with unsynced scripthash writes is
  refused.** Some scripthash index writes land in place over bytes the
  durable index still points at, so a power cut or kill mid-write could
  leave an index that reads as valid and answers address and scripthash
  queries wrongly, with no decode error to trigger a rebuild. Those writes
  now leave `store/scripthash.dirty` until the index is synced, and a
  clean shutdown syncs it. A start with `--sh-index` that finds the file
  stops with a message: start once with `RBITCOIN_SH_FORCE_REBUILD=1` to
  wipe and rebuild the index (address queries answer 503 until it is
  rebuilt at tip), or start without `--sh-index`. The node does not
  rebuild on its own (#2074).
