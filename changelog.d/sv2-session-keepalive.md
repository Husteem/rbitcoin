Fixed

- **A vanished SV2 client frees its session in about a minute.** A client
  that dropped without closing its connection kept its slot and netgroup
  share until a template write failed in the kernel, about 15 minutes, so
  four such drops locked its netgroup out that long. Sessions now use TCP
  keepalive and, on Linux, a 60 s TCP user timeout for unacknowledged
  data. These are best effort: if the system refuses an option, the
  session is still served and the failure is logged at debug.
