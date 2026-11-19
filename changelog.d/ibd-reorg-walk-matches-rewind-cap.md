Fixed

- **IBD selects a heavier fork as deep as the rewind cap allows.** Below
  the work floor a header rewind may disconnect 65,536 blocks, but the walk
  that finds the fork's connected ancestor stopped at 10,000 headers. A
  heavier fork with a longer unconnected span was never selected, and the
  node stayed on the lighter chain. The walk now covers the rewind cap in
  force. An unplanted header-walk tip beyond the widest walk is not walked
  (#1691).
