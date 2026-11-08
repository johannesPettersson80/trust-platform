### Changed

- `eval` (`trust-runtime ctl eval` and the console's watch list) answers the value's type and the value
  as JSON, `{"type": "LREAL", "value": 0.738}`, instead of the runtime's debug text (`LReal(0.738)`):
  numbers as numbers, an enumeration with its `numeric` value, a duration with its nanoseconds and
  its IEC `text` (specification 11). A script that read the text reads the JSON now.
