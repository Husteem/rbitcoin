Security

- **Announced wtxid getdata reads the tip once.** A `getdata` of many
  wtxids this node already announced builds one index of the tip and reuses
  it, including a wtxid the tip does not contain. A later `getdata` for that
  same tip does not read the body again. A full block, a compact block, and
  `getblocktxn` for a stored side block a month behind the best header return
  before that body is read. The connected tip is still served.
