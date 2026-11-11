Added

- **CI runs the workspace test suite on macOS.** The new `macos-test` job
  runs `cargo test --workspace --exclude rbitcoin-bench --features skip-flushes`
  on `macos-26`. Until now, macOS CI ran only the smoke test. The job is not a
  required check yet.
