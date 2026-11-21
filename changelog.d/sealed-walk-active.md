Changed

- **Txid resolve walks sealed `tx.head` segments at the cost of the keys
  still open.** Each sealed segment allocated a candidate list per key of
  the whole wave (up to 256k) and made several passes over all of them,
  even when a handful were still unresolved. The walk now carries the
  unresolved key indices, a segment returns candidates only for keys that
  have some, and identity reads walk only those keys. The pick, fence, and
  fallback rules are unchanged.
