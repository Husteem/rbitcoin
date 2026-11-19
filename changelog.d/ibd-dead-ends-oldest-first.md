Fixed

- **The header walk drops only its oldest dead end at the cap.** Past 1,024
  look-ahead tips that every peer left empty, the walk cleared the whole
  set, so one more mark forgot every earlier one. It now drops only the
  oldest, as the refused-tip set already does.
