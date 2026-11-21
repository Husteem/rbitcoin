Changed

- **Confirm load borrows a pinned parent's output when scripts are not
  run.** Below the assumevalid milestone each spend of a batch-pinned parent
  took the pin's lock, cloned its outputs `Arc` twice, and looked the output
  up twice, then dropped the handle because no script job needed it. It now
  reads value and script under the batch's last-used outputs `Arc`. Script
  jobs still take their own handle.
