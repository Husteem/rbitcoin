Changed

- **A `tx.head` snapshot the store will not trust is not loaded.** At
  open, the footer generation and layout of the open tail snapshot are
  checked before its slots are allocated. A snapshot whose writer died
  before the footer commit no longer costs a zeroed table (128 MiB on
  mainnet) and a full read before it is dropped. A trusted snapshot is read
  straight into the table without zero-filling it first (#1717).
