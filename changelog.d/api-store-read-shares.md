Security

- **One API surface can no longer take every store-read permit.** RPC,
  Electrum, Esplora, and BlindBit drew from `--store-read-budget` with only
  their request caps, so on a 4-core node three concurrent RPC calls, or a
  burst of Electrum or Esplora reads, held all three permits and every
  other API read waited. REST was already held to half. JSON-RPC (long-polls
  and templates included), REST, Electrum, and the Esplora listener with
  BlindBit now each take a share of at most half the budget (at least one
  permit) before a budget permit, so the total stays within the budget.
  With a budget of 1 (`--datadir-hdd`) a share is the whole budget.
