Fixed

- **`submitpackage` names the package's reason in `package_msg`, as in
  Core.** When the package evaluation refuses a 0-fee parent because its
  child leaves the ephemeral anchor unspent, `package_msg` is
  `unspent-dust`; when the package would exceed the cluster limit, it is
  `too-large-cluster`. Both were `transaction failed`. The per-member
  errors are unchanged.
