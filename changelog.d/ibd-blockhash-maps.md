Changed

- **IBD's work-path maps hash block hashes in a few operations.** The
  in-flight, ordered, height, parent, known-header, header-fk, and body
  presence maps used std's SipHash over every 32-byte block hash. The
  per-second hygiene sweep visits every slot of the download window with
  two or three of those lookups, and assign does the same per request.
  They now fold the hash's four words with a rotate and a multiply.
