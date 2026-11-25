Security

- **Esplora `/block-template` builds are shared and reused.** The cached
  template was dropped on every mempool change, and concurrent requests
  that missed it each built their own, so a few hundred parallel requests
  could keep every core busy. As in Core's `getblocktemplate`, a template
  is reused for a mempool change until it is 5 seconds old, and a request
  that finds a build running waits for it, holding no store-read permit,
  and shares its result.
