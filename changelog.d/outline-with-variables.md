### Changed

- The outline of a Structured Text file is a tree that holds variables (specification 14 §6.3.3): a
  namespace's declarations, a CODESYS GVL's globals among them; a configuration's resources, tasks
  and program instances; a POU's variables and parameters, each with its section and type
  (`VAR_INPUT : INT`), and its methods, properties and actions; and an enumeration's values. Each
  entry spans its whole declaration, so the editor's breadcrumbs follow the cursor. It was a flat
  list of POUs and types without variables.
