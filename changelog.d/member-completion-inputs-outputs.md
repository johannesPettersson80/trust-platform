### Fixed

- Completion after `.` on a function-block instance offers the block's inputs and outputs: `Timer.` on a `TON` offers
  `IN`, `PT`, `Q` and `ET`, and a block of another file or of a library offers its own, where it used to offer only
  methods and properties, and nothing for a block that has none. In-outs and protected members stay out where code may
  not name them, as IEC 61131-3 allows inputs to be set and outputs to be read outside the block but not its in-outs.
  Completion asked again while a member's name is being typed (`Timer.E`) keeps offering the members instead of every
  keyword and name in scope.
- Completion after `.` on a namespace's name offers that namespace's declarations from every file of the project:
  `Company.Project.` offers its types, function blocks, functions and nested namespaces, and a CODESYS global
  variable list's name (`GVL_IO.`) offers its globals, while what an `INTERNAL` namespace holds stays out where code
  may not name it. It used to offer nothing there, and nothing after an instance named through its namespace
  (`GVL_IO.gPump.`), which now offers the instance's members.
