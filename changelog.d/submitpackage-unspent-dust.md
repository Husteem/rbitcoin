Fixed

- **`submitpackage` names `unspent-dust` for an anchor its child leaves
  unspent.** When the package evaluation refuses a 0-fee parent because
  its child does not spend the ephemeral anchor, `package_msg` is
  `unspent-dust`, as in Core, instead of `transaction failed`. The
  per-member errors are unchanged.
