### Fixed

- CI clones the OpenOT reference with the job's token instead of anonymously. GitHub refused three
  anonymous clones in a row from a shared runner on 22 November 2026 and failed `main`'s
  Documentation job, which had no fault of its own.
