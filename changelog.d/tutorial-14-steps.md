### Fixed

- Tutorial 14 (deploy and roll back) runs the runtime on the store's `current` link instead of on the path `readlink -f`
  resolves, so a stop and a cold restart load the revision that a deploy or a rollback chose. It walks through each
  step with `trust-runtime ctl` commands and the output of a real run, and CI runs its steps on tutorial 13's project.
