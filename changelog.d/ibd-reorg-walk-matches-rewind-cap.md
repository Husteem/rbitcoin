Fixed

- **IBD selects a heavier fork as deep as the rewind cap allows.** Below
  the work floor a header rewind may disconnect 65,536 blocks, but the walk
  that finds the fork's connected ancestor stopped at 10,000 headers. A
  heavier fork that needed a longer walk was never selected, and the node
  stayed on the lighter chain. The coordinator's and the header thread's
  heavier-fork checks now walk one header past the rewind cap in force
  (65,537 below the floor, 10,000 above it) and pass over a fork deeper
  than that cap instead of ending the check on it. Checks that run under
  the coordinator lock, on a reject or a held side block, still walk
  10,000 headers and leave what they miss to the next coordinator check.
  An unplanted header-walk tip 65,537 or more above the confirmed tip is
  not walked (#1691).
