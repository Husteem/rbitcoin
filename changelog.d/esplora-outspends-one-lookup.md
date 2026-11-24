Security

- **Esplora outspends look up a transaction once, not once per output.**
  `GET /tx/:txid/outspends`, and each transaction of `/txs/outspends`,
  resolved the transaction id again for every output before reading its
  spend. A transaction with tens of thousands of outputs made each request
  repeat that lookup per output. It is now resolved once and each output
  reads only its own spend.
