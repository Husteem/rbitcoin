Fixed

- **An off-lock rewind no longer disconnects a confirmed chain that grew
  during its search.** The pass read the confirmed tip after selecting a
  heavier fork. If confirm extended the chain while the walk ran, that
  extension became the tip the depth check measured, and the rewind
  disconnected it even though the fork was now the lighter chain. The pass
  reads the tip before selection and compares it again under the tip commit
  before disconnecting; a tip that moved leaves the chain alone and the
  fork is selected again on a later turn (#1736).
