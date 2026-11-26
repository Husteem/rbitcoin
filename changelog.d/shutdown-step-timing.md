Added

- **The node logs how long each step of a stop took.** Before `node: clean
  exit`, a `node: shutdown steps` line gives the time since the stop was
  requested, the time since the shutdown tail began, and the time spent
  stopping the listeners, joining the write-behind threads, waiting for tip
  accepts, in the spend checkpoint (including the wait for one already
  running), in the store and mempool flushes, and stopping peer tasks.
  During catch-up the spend checkpoints are most of a stop (#2118). The
  operator setup guide now recommends `TimeoutStopSec=5min` for systemd
  units you write yourself.
