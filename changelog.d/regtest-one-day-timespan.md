Fixed

- **Regtest's difficulty period is 144 blocks, as in Core.** Its target
  timespan was rust-bitcoin's two weeks, so the period was 2016 blocks.
  The minimum-difficulty walk now stops at a multiple of 144.
  `getnetworkhashps` with `nblocks` at or below 0 counts back to the last
  multiple of 144. `getblocktemplate` applies its `mintime` timewarp floor
  at those heights too. With `--regtest-pow-bits`, a block at a multiple of
  144 whose parent carries the pow limit now keeps the pow limit, so a
  regtest chain mined with that flag may need to be mined again (#1775).
