Security

- **One network can no longer fill the SV2 template provider.** Eight
  silent clients used to hold every session, and since SRI pools are
  silent between shares no idle deadline can tell them apart. A netgroup
  (IPv4 `/16`, IPv6 `/32`) now holds at most 4 of the 8 sessions; the
  next connection from it is closed before the handshake and existing
  sessions are not touched, and the refusal logs at most once a second.
  A fifth client from one `/16`, such as miners behind one NAT or one
  tunnel endpoint on IPv4 loopback, is refused too.
- **An SV2 session gets a template's transactions at most twice.**
  Each `RequestTransactionData` reply serializes every template
  transaction. A third request for the same template gets
  `RequestTransactionData.Error` with code `request-limit-reached`, and
  the session stays open. The bound is per template and per session: a client that
  reconnects gets new templates, and asking for those serializes the
  same transactions again. The netgroup share above limits how many
  such sessions run at once.
