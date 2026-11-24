Security

- **An Esplora newest-first history page takes `--max-sh-creates` from the
  script's create count.** The cap ran only when a page filled. A cursor
  deep in a large address's history, with fewer than a page of creates
  below it, walked every newer create and skipped the cap. Each page also
  loaded the whole posting list first. A script with more creates than the
  page limit is now checked against the cap and the heavy turn before its
  list is read, at any cursor.
