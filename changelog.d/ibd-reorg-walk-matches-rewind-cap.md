Fixed

- **IBD selects a heavier fork as deep as the rewind cap allows.** Below
  the work floor a header rewind may disconnect 65,536 blocks, but the walk
  that finds the fork's connected ancestor stopped at 10,000 headers. A
  heavier fork that competes at the next height and needs a longer walk
  was never selected, and the node stayed on the lighter chain. Below the
  floor the walk now reaches 65,537 headers. It keeps 10,000 once the
  cached chain work shows the confirmed tip at the floor; while that cache
  is stale the walk stays at 65,537. An unplanted header-walk tip 65,537 or
  more above the confirmed tip is not walked (#1691).
