### Fixed
- Fit (`identify --recording`, Open recording in Control Studio) refits any live recording the study keeps, not only the
  recording of its Identify result: each live recording keeps the request it was taken with in the study. A recording kept
  before this change has none and is refused as before (specification 35, B3.6).
