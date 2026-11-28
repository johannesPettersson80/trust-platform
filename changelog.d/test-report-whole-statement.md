### Fixed

- A failed test whose assertion is written over several lines is now reported with the whole statement: the human
  output's `source`, TAP's `# source:`, JUnit's `source:` and JSON's `failure.source` showed its first line alone,
  such as `ASSERT_EQUAL(`. The statement's lines are trimmed and joined by one space. A statement that holds others,
  such as an `IF`, is shown up to its first held statement, so that an error in its condition shows the condition and
  not the body. On a device, `cycle.step` names the statement's last line in `error_at` as `last_line`.
