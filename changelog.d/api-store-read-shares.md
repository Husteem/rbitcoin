Security

- **One API surface can no longer take every store-read permit.** RPC,
  Electrum, Esplora, and BlindBit drew from `--store-read-budget` with only
  their request caps, so on a 4-core node three concurrent RPC calls, or a
  burst of Electrum or Esplora reads, held all three permits and every
  other API read waited. REST was already held to half the budget.
  JSON-RPC (long-polls and templates included), REST, Electrum, and the
  Esplora listener with BlindBit are now each sure of an even split of the
  budget among the surfaces the node serves (at least one permit, when the
  budget has one for each), and a busy surface borrows idle permits up to
  one short of the budget, or all of it when it is the only surface on. A
  freed permit goes first to a surface below its split, in turn when
  several are; running reads are not taken back, so a surface whose
  permits are lent out waits for them to finish. A surface that is off
  reserves nothing. With a budget of 1 (`--datadir-hdd`) the surfaces that
  wait take turns at the one permit.
