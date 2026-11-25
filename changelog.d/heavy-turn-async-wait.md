Security

- **A heavy address history no longer freezes every API read while it
  waits.** A request for a script above `--heavy-sh-creates` waited for
  the node's one heavy turn on a blocking-pool thread while holding a
  store-read permit. The pool is the core count and the budget one less,
  so a few concurrent big-address requests (the documented explorer
  setup with `--max-sh-creates 0`) stalled Electrum, Esplora, and every
  other API read. The wait now happens on the request's task with no
  thread and no permit held. Heavy joins still run one at a time.
