Fixed

- **IBD selects a heavier fork as deep as the rewind cap allows.** Below
  the work floor a header rewind may disconnect 65,536 blocks, but the walk
  that finds the fork's connected ancestor stopped at 10,000 headers. A
  heavier fork that needed a longer walk was never selected, and the node
  stayed on the lighter chain. The coordinator's and the header thread's
  heavier-fork checks now walk as far as the rewind cap in force needs
  (65,537 headers below the floor; 10,000 above it, as before), pass over
  a fork deeper than that cap, and rewind to the candidate with the most
  work rather than the first heavier one. Checks that run under the
  coordinator lock, on a reject or a held side block, still walk 10,000
  headers. Below the floor they leave the choice to the next coordinator
  check when more than one candidate is in play, and they hand over what
  they miss and any fork whose rewind could not run yet. An unplanted
  header-walk tip 65,537 or more above the confirmed tip is not walked
  (#1691).
