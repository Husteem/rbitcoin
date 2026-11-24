Security

- **An oversized Electrum batch is refused before it is parsed.** A batch
  line was turned into a full JSON tree before the 345-item limit was
  checked, so a 1 MiB line of tiny items took about 16 MiB per connection
  before it was refused. The items are now counted without building
  anything, and a batch past the limit gets `batch too large` at once.
