Changed

- **A `pub` item nothing names fails `qc`.** rustc does not report a dead
  `pub` item in a library crate, so the last caller could go and leave it
  behind. `./scripts/unused-pub.sh` now fails on a plain `pub` item in
  `src/` that no production file, test, or fuzz target names. Every such
  item on master is deleted, with the store's write-backend selector that
  no write path read and a directory-sync fault hook nothing could arm.
