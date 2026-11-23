Security

- **Replacement and cluster checks run before script checks.** A
  transaction that conflicts with the mempool but pays too little to
  replace it, or that would overfill its cluster, used to be refused only
  after its signatures and scripts were verified. A peer could resend such
  a transaction and make the node verify it again each time. Both checks
  now run first, as in Core's PreChecks, so the refusal costs no script
  work. A transaction that fails both is now reported with the replacement
  or cluster error, as in Core.
