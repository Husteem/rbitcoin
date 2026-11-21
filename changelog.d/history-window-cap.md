Security

- **A history window and a full newest-first page stay inside
  `--max-sh-creates`.** An Electrum `get_history` `from_height` window and a
  newest-first page whose bound sits inside the 8,064-block history window
  both read every create. They now take the same create cap and heavy-join
  turn as an unpaged read. A page that closes before reading every create is
  still served.
