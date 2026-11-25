Fixed

- **A header-walk rewind gives back the lead bytes its dropped plan reserved.**
  `ConfirmFeed::drop_plan` keeps a height's reservation while the in-channel
  write may still commit, but a stale batch that drops before writing sends no
  Accepted event to release it. Those bytes stayed charged to the unproven
  lead, so a later body at the same height skipped the byte charge even with
  the lead spent. Dropping a stale claim now returns its reservation, both
  when load drops the plan and when write drops it.
