### Fixed
- Tutorial 15 (multi-PLC mesh) shares a value between two runtimes as written: its configurations
  held an empty mesh `auth_token`, which the runtime refuses, published a name the program does not
  have, subscribed under the discovery name instead of the publisher's resource name, and sent the
  reader to web pages that no longer exist. It now shares PLC-B's lamp into a variable of PLC-A and
  finds PLC-B through discovery. The multi-PLC guide, the developer and operator guides and the mesh
  page follow.
