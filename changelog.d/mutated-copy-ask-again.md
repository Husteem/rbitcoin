Fixed

- **Tip-follow asks a `--connect` or noban peer again for a block it sent
  as a mutated copy.** The copy was dropped and the request forgotten, and
  nothing asked for the block again until a later message from that peer,
  so a single `--connect` peer could leave the node one block behind. Core
  asks again at once. A copy whose witness bytes the block does not commit
  to is now handled as mutated too, as Core's `IsBlockMutated` does: the
  sender is punished like the other mutations, and the block is not marked
  invalid (#1771).
