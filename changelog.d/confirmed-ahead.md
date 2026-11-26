Fixed
- **`confirmed[]` survives a torn page, and open refuses a hole in it.**
  Every `confirmed[]` flush rewrites the length in page 0 and the 4 KiB block
  that holds the previous tip, with no image. A torn page 0 failed open with
  `BadMagic`. A torn tip block zeroed committed heights; the tip-seal clamp
  could leave that null run untrimmed and shrink the tip onto a null slot.
  A null below the revalidation window was skipped by the height fence, and
  open then cleared the strong bits of that block's transactions. The flush
  now writes a synced `confirmed.body.ahead` image first and open restores
  it. Open clamps to the seal before it trims null slots, and refuses any
  other null slot below the tip before it builds the fence. An array flush
  whose shrink ends on a block boundary still images page 0, which covers
  `header_txs` too.
