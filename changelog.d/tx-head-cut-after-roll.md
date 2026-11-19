Fixed

- **A crash just after a `tx.head` roll no longer fails the next start.**
  When the open cut dropped the new tail, the segment whose seal was still
  running became the tail without a table, and open failed with
  `tx.head insert: open missing OA`. Open now rebuilds that segment from
  the archive.
