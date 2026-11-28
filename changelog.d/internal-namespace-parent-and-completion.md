### Changed

- The parent namespace of an `INTERNAL` namespace now reaches the public elements of that namespace, as IEC 61131-3
  Figure 29 allows: an `INTERNAL` namespace at the top is used from code outside any namespace, and `Plant`'s
  `INTERNAL` namespace `Plant.Hidden` from `Plant`; any other namespace is still refused. Before, only code inside the
  `INTERNAL` namespace reached it.

### Fixed

- Completion no longer offers an `INTERNAL` element of a namespace outside its namespace, after the namespace's name
  and `.` or by its name through a `USING`, where naming it is an error.
