Fixed

- **A rewind no longer disconnects a confirmed chain that grew during its
  search.** The rewind read the confirmed tip after selecting a heavier
  fork. If confirm extended the chain while the walk ran, that extension
  became the tip the depth check measured, and the rewind disconnected it
  even though the fork was now the lighter chain. Both the off-lock pass and
  the pass under the coordinator lock now read the tip before selection,
  compare it again under the tip commit before disconnecting, and hold that
  commit through the commit. A path that no longer disconnects is left for
  confirm, and a moved tip selects again on a later turn (#1736).
