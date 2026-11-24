# 093 — History catch-up scans every block in the window

**Severity:** high
**Status:** fixed
**Found by:** bobberb and muse, 2026-10-18

`get_history` with `from_height` of 0 is the capped scripthash join. `from_height` of 1 or more is a catch-up. It loaded the posting and read `input.body` for every block in the clipped window. It now takes the same `--max-sh-creates` refusal and heavy-join turn as an unpaged join, and finds the spends in the window from the spend slots of the script's outputs in every create at or below the window's top, so the work follows the create count, not the window's blocks. A window wider than 8,064 blocks is refused before that read. A one-block catch-up still returns the spend when the script is under the cap.

**Regression:** `rbitcoin-query` `history_window_wider_than_eight_weeks_is_refused`, `height_window_reads_spend_slots_not_blocks`, `sh_history_caps`, and `rbitcoin-electrum` `from_height_window_obeys_the_create_cap`.
