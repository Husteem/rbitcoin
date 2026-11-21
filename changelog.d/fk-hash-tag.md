Changed

- **Integer-keyed maps no longer compare every slot on a probe.** The
  identity hasher behind `U64Map`, `U64Set`, `FkMap`, `FkSet`, and
  `U32Map` returned the raw key. hashbrown filters a probe group by a tag
  from the top 7 bits of the hash, and every fk, height, and offset is far
  below 2^57, so all entries shared one tag and each probe compared every
  full slot in its group. The hash now keeps the key in the low 57 bits, so
  buckets and locality are unchanged, and puts a multiplied tag in the top
  7. These maps sit on every confirm thread (pins, stamp, structural spend
  checks).
