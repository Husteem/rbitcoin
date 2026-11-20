Fixed

- **Tip-follow asks a `--connect` or noban peer again for a block it sent
  as a mutated copy.** The copy was dropped and the request forgotten, and
  nothing asked for the block again until a later message from that peer,
  so a single `--connect` peer could leave the node one block behind. Core
  asks again at once. A copy whose witness bytes the block does not commit
  to is handled as mutated, as Core's `IsBlockMutated` does: the sender is
  punished like the other mutations, and the block is not marked invalid.
  A signet block with no witness commitment is a consensus failure (Core
  `CheckSignetBlockSolution`, `bad-signet-blksig`): the hash is marked
  invalid and is not asked for again. A signet body that still carries
  witness data and has no commitment stays a mutated copy, because Core
  reports `unexpected-witness` first (#1771).
