Fixed

- **`getpeerinfo.bytessent_per_msg` counts what was sent.** It was always
  `{}`: nothing wrote the per-message map. The session writer now counts
  each message it puts on the wire, as payload plus the 24-byte v1 header,
  the same rule `bytesrecv_per_msg` uses. Messages sent during the
  handshake, before the writer starts, are not counted.
