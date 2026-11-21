Changed

- **A `/metrics` scrape no longer walks the process page tables.**
  `process_resident_memory_bytes` read `/proc/self/smaps_rollup`, which
  the kernel builds by walking every mapped page, to report the resident
  size `/proc/self/status` already gives. It now reads `status` only.
