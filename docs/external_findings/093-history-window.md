# 093 — History catch-up scans every block in the window

**Severity:** high
**Status:** fixed
**Found by:** bobberb and muse, 2026-10-18

`get_history` with `from_height` of 0 is the capped scripthash join. `from_height` of 1 or more is a catch-up: it loads the posting and reads `input.body` for every block in the clipped window. It now takes the same `--max-sh-creates` refusal and heavy-join turn as an unpaged join, because it reads every create to decide which spends in the window pay the script. A window wider than 8,064 blocks is refused before that scan. A one-block catch-up still returns the spend when the script is under the cap.

**Regression:** `rbitcoin-query` `history_window_wider_than_eight_weeks_is_refused`, `sh_history_caps`, and `rbitcoin-electrum` `from_height_window_obeys_the_create_cap`.
