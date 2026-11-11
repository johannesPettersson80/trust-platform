### Fixed

- The assignment attempt `?=` between two interfaces that are not related, `interf2 ?= interf1` as IEC 61131-3
  §6.6.6.7.3 shows it, and from an interface to a reference of a class that does not implement it now type-check, as
  specification 06 gives them: the class of the instance decides at run time. They were refused with E201.
