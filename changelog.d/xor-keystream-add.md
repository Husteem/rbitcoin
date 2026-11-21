Changed

- **Script and witness XOR runs without a multiply per byte.** Every
  stored script and witness byte is XORed with a keystream whose mix byte
  came from a 64-bit multiply of its offset. Consecutive offsets differ by
  one, so the walk now adds the constant instead; the bytes are the same.
