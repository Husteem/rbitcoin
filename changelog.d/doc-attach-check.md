Changed

- **A doc comment left on the wrong item fails `qc`.** Inserting an item
  between a `///` block or an attribute and its item, or deleting an item
  and keeping its block, moves that block onto another item with no
  warning. `./scripts/doc-attach.sh` compares a pull request with its base
  and fails on both. Three such blocks on master are back on their items,
  one of them a `#[doc(hidden)]` that had moved from a public function to a
  private one.
