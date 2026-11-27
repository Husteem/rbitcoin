Fixed

- **`rbitcoin-cli --stdin` can supply the method name.** `echo
  getblockcount | rbitcoin-cli --stdin` failed with usage because stdin
  lines were only appended as parameters after the method was chosen
  from argv. Stdin lines are now taken in order, so the first line is
  the method when argv has none, and stdin lines stay parameters when
  argv already names a method.
