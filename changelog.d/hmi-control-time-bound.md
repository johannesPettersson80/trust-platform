### Fixed

- An HMI page whose control declares a `pulse` or `timeout` above 600 seconds is refused by the page check
  (specification 39). Before, a time of years could panic the runtime's command handling and keep one of the
  followed-command slots for good, so that after 64 such commands no confirmation or remote pulse end was reported.
  A followed command now gives its slot back however its follow ends.
