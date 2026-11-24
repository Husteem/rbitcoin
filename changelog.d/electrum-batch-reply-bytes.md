Security

- **An Electrum batch holds at most 1 MiB of unwritten replies.** A batch
  ran up to half the blocking pool's reads at once, and the replies that
  finished waited in memory while the first one was written. A client that
  asked for a large transaction several times in one batch and read
  slowly held several copies of it per connection, which across many
  connections could take gigabytes. A finished reply that would take a
  connection past 1 MiB beside the others is now dropped and read again
  when its turn to be written comes.
