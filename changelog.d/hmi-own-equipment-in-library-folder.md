### Fixed

- The HMI Builder's My equipment lists a project's own function blocks when a library folder holds the
  project, as `Plant = { path = "../.." }` holds each Plant example. Before, every file of such a project
  was taken for the library's, and the list was empty. A library inside the project still keeps its blocks
  out (specification 39 §21.9, by the deepest-folder rule of specification 14 §7.6).
