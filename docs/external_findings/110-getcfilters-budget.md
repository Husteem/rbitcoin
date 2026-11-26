# 110 — `getcfilters` kept queueing after the send budget

**Severity:** medium
**Status:** fixed
**Found by:** @MrHodl, 2026-11-10

The send-budget gate ran once, before the handler. `getcfilters` could
then queue every filter in the request, each one past the cap.
`getcfcheckpt` also read one filter-header row per checkpoint height.

`getcfilters` queued the filter that crossed the budget and then stopped,
which cut a 1,000-block range short (#2078). Since #2106 it parks the rest
of the range and sends it once the writer drains. `getcfcheckpt` reads the checkpoint range once and still returns the
header at every 1,000th height.

**Regression:** `rbitcoin-net` `peer::getcfilters_parks_the_rest_past_the_send_budget`, `peer::getcfcheckpt_returns_a_header_every_thousand_heights`.
