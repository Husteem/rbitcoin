Changed

- **Archive planning finds a spend's block by binary search.** For every
  input of a batch, the plan checked whether its parent sits in the same
  block by scanning the batch's per-block ranges from the start. The
  ranges are in height order, so it now searches them.
