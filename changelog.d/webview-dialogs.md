### Fixed

- Ladder's Find and replace and the Statechart's Add and Delete action mapping did nothing: they asked through browser
  dialogs, which VS Code's webviews ignore. They now ask in VS Code's input boxes and warnings, Escape cancelling, and
  the language server refuses a new action's name that is blank or already in use (specification 25 §11.4).
