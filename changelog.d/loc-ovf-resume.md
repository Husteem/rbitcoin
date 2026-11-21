Changed

- **`create.loc` window decodes no longer start over at an overflow
  slot.** The 8-slot fast decode stopped at the first slot that needs an
  overflow row, and the scalar decode then redid the whole window. Now
  only that 8-slot chunk takes the scalar decode, and the fast decode goes
  on after it.
