Security

- **The discourage list keeps recording when full.** Misbehaving
  addresses, rate-limit strikes and paused netgroups were kept in maps of
  at most 10,000 entries that ignored new keys once full, so one IPv6
  network could fill the list and nobody who misbehaved in the next day
  was kept out. A new entry at the cap now evicts the oldest, as Core's
  rolling filters do.
