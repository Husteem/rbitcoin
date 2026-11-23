Changed

- **The nightly image guard is a tested script.** The check that keeps
  `docker-publish` from moving `nightly` back to an older commit moved out
  of `release.yml` into `scripts/nightly-guard.sh`, with a self-test in
  `qc` that replays each way it broke before. No behavior change.
