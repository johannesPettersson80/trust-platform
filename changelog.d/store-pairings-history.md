### Fixed

- A runtime started on a program store's `current` link now keeps its pairings (`pairings.json`)
  and its historian's history and alert files in the store folder, as it keeps its retained values,
  so an install, a deployment or a rollback no longer starts them empty. The first start of this
  version moves such files that an earlier version left in the `current` bundle into the store
  folder.
