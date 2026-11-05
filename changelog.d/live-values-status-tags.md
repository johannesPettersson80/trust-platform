### Fixed

- Live Values keeps the runtime's name and endpoint in its header when its debug session ends or another debug
  session becomes active, instead of showing an unnamed target; a lost-connection or debugging-disabled message goes
  once live values arrive again; and a release on a target that stays armed keeps saying so instead of disappearing
  after 5 seconds (specification 25 §4.2, §4.4).
