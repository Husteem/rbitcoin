Security

- **One Electrum connection runs at most 3 silent-payment history scans at
  once.** The process runs 32 scans, and each connection could subscribe
  16 addresses that scanned at the same time, so two connections could
  take every slot for hours and refuse everyone else. A connection's
  fourth and later subscribes are now accepted and scanned in turn with
  the slots its own scans finish with. Filling every slot takes 11
  connections instead of 2.
