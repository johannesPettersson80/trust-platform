### Added

- The runtime's log now says which mesh peers come, go or stay away: `mesh_peer_joined` when a runtime joins the mesh,
  the warning `mesh_peer_left` when it leaves, and, 5 s after the mesh started, the warning `mesh_peer_missing` for
  each peer that a subscription names and that is not in the mesh, with the subscriptions that wait for it and what
  to check. A subscription that waited for a peer with another `auth_token`, or for a peer named otherwise than by
  its resource name, used to receive nothing and log nothing.
