Added

- **The node logs how long each step of a stop took.** After `node: mempool
  flushed`, a `node: shutdown steps` line gives the time since the signal and
  the time spent joining the write-behind threads, waiting for tip accepts,
  in the final spend checkpoint, and in the store and mempool flushes. During
  catch-up the spend checkpoint is most of a stop (#2118). The mainnet
  operator guide now recommends `TimeoutStopSec=5min` for systemd units you
  write yourself.
