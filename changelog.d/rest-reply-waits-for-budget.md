Fixed

- **A built REST reply waits for its share of the byte budget instead of
  being refused at once.** A built reply that did not fit the 64 MiB REST
  reply budget was answered 503 at once, so a reply over 64 MiB (such as
  `/rest/mempool/contents.json` on a full mempool) failed whenever any
  other REST reply was still unwritten. A built reply now waits up to 30
  seconds for its bytes, and takes them ahead of later requests, which
  are 503 before anything is built while it waits. After 30 seconds it is
  dropped and answered 503 as before (#2088).
