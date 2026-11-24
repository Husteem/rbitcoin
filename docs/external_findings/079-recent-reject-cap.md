# 079 — Recent-reject set stops at 4096

**Severity:** low
**Status:** fixed
**Found by:** Stephan Livera, 2026-10-02 (L6)

The recent-reject set cleared itself when it reached the cap, so it could keep growing. It then stopped at 4096 entries, which let cheap rejects switch it off until the next block (#1949). It now rolls: up to 120,000 entries, as Core, and a full set drops its oldest entry.

**Regression:** `rbitcoin-net` `recent_rejects_roll_instead_of_refusing`.
