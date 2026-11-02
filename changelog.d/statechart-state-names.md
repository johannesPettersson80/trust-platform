### Fixed

- A new state in the Statechart editor gets a name no other state has. Before, Add state named it after the number
  of states, so after a deletion it could repeat a name, and saving the chart kept only one of the two states,
  losing the other with its transitions. Saving now also refuses, with the reason, a chart in which two states share
  a name or more than one state is initial, instead of keeping one of them (specification 25 §11.4).
