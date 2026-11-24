Security

- **A peer can no longer grow `bytesrecv_per_msg` without limit.** Every
  received command name got its own key, kept for the life of the
  connection, so a peer that sent made-up commands grew the node's memory
  by about a gigabyte an hour. A command outside Core's message list now
  counts as `*other*`, as in Core.
