Changed

- **Library tests are out of `src/` again.** The inline `#[cfg(test)]`
  modules that returned after the suite move now live in each crate's
  `tests/suite` target, where the default `cargo test` runs them. Tests
  that only reached a private helper were dropped; the public journey
  already covered that line. `./scripts/src-test-modules.sh` fails `qc`
  when `#[cfg(...test...)]` returns to a production `src/` file.
