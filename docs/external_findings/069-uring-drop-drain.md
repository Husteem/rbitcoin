# 069 — io_uring drop does not free a buffer still in the kernel

**Severity:** high
**Status:** fixed
**Found by:** Stephan Livera, 2026-10-02 (M9)

Dropping an io_uring session that had stopped completing freed buffers the kernel could still write. Drop uses the same drain hard cap as the fail-closed path and does not shorten that cap. A drop drain that leaves an id pending aborts, and so does a drain that hits the cap.

**Regression:** `rbitcoin-store` `drain_guard_drop_with_leftover_pending_aborts` (leftover id after the drain), `drain_all_with_no_completions_for_the_hard_cap_aborts` (hard cap).
