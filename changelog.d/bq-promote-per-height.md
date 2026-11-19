Changed

- **IBD drops each block's raw bytes as soon as lookup decodes it.** A
  lookup wave used to keep every block's raw wire beside its decoded copy
  until the wave's parent-txid read finished: roughly 25–60 MB per wave,
  for as long as that read takes. A cancelled wave now keeps its decoded
  blocks queued, so the next wave does not decode them again.
