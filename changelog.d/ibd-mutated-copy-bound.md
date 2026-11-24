Fixed

- **IBD asks for an equal-work sibling block and stops re-getting a body that
  only arrives mutated.** A `--connect` or noban peer that answered a block
  getdata with a mutated copy re-opened that hash on every copy, so `ordered`
  never emptied and IBD never left catch-up; the sibling block, whose header
  breaks no rule, was never asked for. The sibling is now recorded and asked
  for once the occupying header is abandoned (Core asks at once, on
  `nChainWork >=`), and after three copies from a peer IBD keeps, the header
  is taken off the path instead of re-opened (#1811).
