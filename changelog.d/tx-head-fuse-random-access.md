Changed

- **Sealed transaction filters request random-access advice on Linux.**
  This suppresses mmap readahead around random fingerprint probes, reducing
  the pages loaded on a cold lookup.
