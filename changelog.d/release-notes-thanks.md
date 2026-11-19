Fixed

- **The GitHub Release thanks contributors again.** `release-notes.sh`
  started its range at the newest tag, which in `release.yml` is the tag
  being released, so the automatic thanks line was empty (0.9.0's Release
  has been corrected by hand). It now starts at the newest release tag the
  commit descends from, and the Release job fetches full history. The
  release playbook also runs `release.yml` on the ship branch before
  merge, so a Windows, Darwin, image, or NixOS VM failure shows before the
  tag is pushed.
