Security

- **A history window reads spends per create, not per block.** An Electrum
  `get_history` with a `from_height`, or an Esplora newest page whose page
  bound fell within 8,064 blocks, read the inputs of every block in that
  window to find spends. A script with one output made every such request
  cost up to 8,064 blocks of reads. Those spends now come from the spend
  slots of the script's own outputs, so the work follows the script's
  create count, which `--max-sh-creates` already bounds.
