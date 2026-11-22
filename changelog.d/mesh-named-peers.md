### Security

- A runtime's mesh links only to the peers its `[runtime.mesh] connect` entries name and to the
  runtimes that connect to its `listen` endpoint (specification 11 §6.12). It used to join every mesh
  that answered its multicast on the network, and every peer that a linked peer knew, so without
  `auth_token` a runtime on the same network could read the published values and publish into the
  subscribed ones. A setup that relied on its runtimes finding each other names each peer in `connect`
  on one of the two sides; the runtime cloud quickstart's runtime-a now connects to runtime-b.
