Fixed

- **A scripthash index rebuild no longer serves a short history for a
  moment.** A rebuild let Electrum and Esplora readers back in before it
  re-queued the blocks that connected while it collected, so a
  subscribed wallet could be sent a status, and `get_history` a list,
  missing payments confirmed during the rebuild until the next write
  caught up. Readers are now admitted only after those blocks are
  queued.
