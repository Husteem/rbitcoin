Changed

- **Single confirmed-transaction lookups skip older index segments.** Once a
  transaction ID matches a row on the active chain, the lookup skips the
  remaining segments.
