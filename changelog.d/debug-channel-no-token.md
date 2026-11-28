### Security

- The "truST Debug" output channel no longer shows control tokens. Every attach session wrote its `authToken` there,
  in the attach request that the debug adapter tracker logged in full, and a session's start and end lines logged its
  whole configuration. The channel now names each session by its name, type and request, and each Debug Adapter
  Protocol message by its kind, command or event, sequence number, success and error text, never its arguments or
  body.
