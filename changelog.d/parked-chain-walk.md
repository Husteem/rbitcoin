Security

- **A parked header chain is not walked again on the next message.** A peer
  could send 8,000 headers that do not connect, then one more header on
  that chain. Each later message walked every parked header and allocated
  a path through them. A header parked off any header this node has is
  remembered with that distance. The next message stops there, so it costs
  the headers it brought. A later page that reaches a header this node has
  drops those marks, including marks parked on a header in that page.
