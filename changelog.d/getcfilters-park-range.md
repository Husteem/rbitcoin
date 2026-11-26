Fixed

- **A `getcfilters` range past 4 MiB is sent whole.** A compact-filter
  reply stopped at the 4 MiB per-peer send budget with no `notfound`, so
  a request for 1,000 recent mainnet blocks got the first few hundred
  filters and then nothing, and BIP157 clients such as Neutrino and Kyoto
  waited until their timeout or dropped the peer. The rest of the range
  is now parked and sent in order once the writer drains, as a paused
  `getdata` is, and the session reads no new message until it is sent.
