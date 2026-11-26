Fixed

- **An Electrum outpoint subscribe no longer cancels a pending restatus.**
  When a status read failed at a tip, or tip events lagged, the next tip
  was meant to restatus every subscribed outpoint. A
  `blockchain.outpoint.subscribe` in between reset that, so the next block
  restatused only the outpoints it touched, and a spend made in the failed
  block was never pushed to that connection (#2081).
