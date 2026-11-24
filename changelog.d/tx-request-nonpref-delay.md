Security

- **Inbound peers no longer win every transaction request.** The node
  asked the first peer to announce a transaction for it at once and waited
  up to a minute before asking anyone else, so an inbound peer that
  announced first and never answered could delay most new transactions by
  about a minute. As in Core, an inbound peer without `noban` is now asked
  only 2 seconds after its announcement, so an outbound peer that
  announces in that time is asked instead.
