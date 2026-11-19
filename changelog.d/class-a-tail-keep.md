Fixed

- **A torn Class A tail no longer drops creates the tip already names.** An append copies a synced partial 4 KiB block aside before rewriting it, and open restores that copy when the block tore to zeroes. A rewrite waits out a Class A sync that is still removing the previous copy.
