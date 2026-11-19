Fixed

- **IBD selects a heavier fork as deep as the rewind cap allows.** Below
  the work floor a header rewind may disconnect 65,536 blocks, but the walk
  that finds the fork's connected ancestor stopped at 10,000 headers. A
  heavier fork that competes at the next height and needs a longer walk
  was never selected, and the node stayed on the lighter chain. Below the
  floor the walk now reaches 65,537 headers; above it, the walk keeps
  10,000. An unplanted header-walk tip 65,537 or more above the confirmed
  tip is not walked (#1691).
