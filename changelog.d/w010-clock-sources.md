### Changed

- W010 tracks clock sources instead of declarations (issue #250). A
  TIME, LTIME, DATE, LDATE, TOD or DT declaration no longer warns. A
  call of CURRENT_DT() warns, and so does the call site of a function,
  method, function block or program whose body reaches one, directly or
  through the POUs it calls; the message names the source: "CURRENT_DT
  reads the host clock, which replay does not control". TIME() and what
  is built on it (T_PLC_MS, T_PLC_US, RTC_MS, RTC_2, the timers) stays
  warning-free, as does a value whose source cannot be traced, such as a
  body-less library stub (specification 28 section 5).
