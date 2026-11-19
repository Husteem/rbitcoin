Fixed

- **IBD selects a heavier fork as deep as the rewind cap allows.** Below
  the work floor a header rewind may disconnect 65,536 blocks, but the walk
  that finds the fork's connected ancestor stopped at 10,000 headers. A
  heavier fork with a longer unconnected span was never selected, and the
  node stayed on the lighter chain. The walk now reaches 65,537 headers,
  one past the widest rewind cap. An unplanted header-walk tip that far
  above the confirmed tip is not walked (#1691).
