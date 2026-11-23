### Fixed

- The documentation site keeps the numbered steps of 14 pages together. These pages included six language and
  runtime specifications, the deterministic harness, three guides and the communication, Rust service, plant demo
  and tutorial READMEs. Each of them had ended its list at a step's code, table or second paragraph, so the later
  steps were numbered from 1 again or ran into one paragraph. A check now fails a change that would break a page's
  steps that way.
