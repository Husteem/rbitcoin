Security

- **Messages sent during the handshake get the same size checks as after
  it.** Before `verack`, frames were decoded without the checks that
  refuse an oversized `headers`, inventory or transaction count before
  allocating, so a peer could make the node allocate for counts its
  payload could never hold. Such a frame now closes the connection, as it
  does after the handshake.
