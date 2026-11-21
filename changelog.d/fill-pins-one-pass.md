Changed

- **Confirm load chooses each parent's plan pin in one pass.** It walked
  the batch's parents three times (batch pins, in-flight creates, recent
  stamped pins), checking a growing map each time. One pass now takes the
  first source that holds the parent, into a map sized for the batch.
  `ibd: perf_full` drops `load_pin_recent_outs_ms`, the third walk's wall,
  which `load_thin_fill_ms` already covers.
