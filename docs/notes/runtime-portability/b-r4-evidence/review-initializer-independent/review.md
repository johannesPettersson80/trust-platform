# Independent initializer boundary review

Accepted by the call/collection agent by source inspection only. No builds, tests, formatters, link or hardware execution.

The finish_initializer body is identical to the old evaluator tail. The call occurs after lexical locals are restored. Outcome, staging result ID, lexical owner, instance and instance backups are transferred unchanged. Destination selection, charge ordering, promotion, rollback ownership, context pop, frame release and owned-instance retirement are preserved. The targeted noinline boundary keeps finalization temporaries out of the evaluator frame but is not credited with measured stack savings.

Native additions preserve the existing forged-store assertion and check staging/type/frame cleanup plus subsequent reuse. The separate boundary test exercises explicit BOOL initialization at depth 3 under max depth 4, rejects depth 4, then reuses the same state. API and imports were checked against current storage/prepared structures. No reduction of admitted call depth or proof of worst-case native stack is claimed. Remaining initializer/construction recursion is explicitly retained for physical measurement.
