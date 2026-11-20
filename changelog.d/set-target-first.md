### Fixed
- `trust-runtime ctl set` with a target that lacks its `global:` or `retain:` prefix, such as
  `ctl set Setpoint 12.5`, now says `unsupported target (use global:<name> or retain:<name>)`, as a
  force does. It answered `unsupported value '12.5'` for a decimal value, which blamed the value.
