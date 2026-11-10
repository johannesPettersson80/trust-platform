### Fixed

- A runtime started on a program store's `current` link lost its retained values at the first warm
  restart after an install, a deployment or a rollback: its retain file stayed in the bundle it had
  started on, and the new bundle held none. A relative retain path now lies in the store folder, and
  the first start of this version moves a retain file left in the `current` bundle there.
