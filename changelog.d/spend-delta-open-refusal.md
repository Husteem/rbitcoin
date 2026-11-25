Fixed

- **A schema-30 datadir that lost `spent.delta/` refuses to open instead
  of forgetting spends.** Open refused a populated `spent.body` with no
  delta only when the file was shorter than its published end. A sparse
  body whose last written page reached that end passed as a 0.9.x body
  and was adopted, leaving every spend the delta held unspent. Open now
  decides by the schema `meta` held when it started: only a datadir from
  before schema 30 adopts its body.
