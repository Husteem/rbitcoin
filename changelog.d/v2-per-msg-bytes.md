Fixed

- **`getpeerinfo` counts a v2 message as Core does.** `bytesrecv_per_msg` and
  `bytessent_per_msg` added the 24-byte v1 header to the payload for every
  peer, so a message with a short id read 3 bytes high and one in the long
  form (`sendheaders`, `verack`, `version`) read 9 bytes low. A v2 message is
  now its BIP324 contents plus the 20-byte cipher expansion, and the sums
  follow. A v1 peer keeps payload plus 24.
