Changed

- **Confirm queues measure a batch's wire size once.** The load, script,
  and write queue accounting each walked every transaction of every block
  in a batch (`Block::total_size`) to log its wire bytes, about four walks
  per batch. Load now measures it once and the batch carries it.
