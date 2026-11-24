Security

- **A peer that sends invalid headers is disconnected.** A `headers`
  message whose headers connect to our chain but fail the header checks
  (proof of work, difficulty bits, or a time at or below the median) was
  dropped without scoring the peer, so it could resend such batches
  forever and keep its slot. As in Core, that peer is now disconnected
  and discouraged; manual and `noban` peers stay. A time too far in the
  future is still not the peer's fault.
