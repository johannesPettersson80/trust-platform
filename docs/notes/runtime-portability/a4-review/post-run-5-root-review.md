# Post-run-5 root correction source review

Source-only review of the root agent's state grouping, fault identity/classification,
new STBC 2.0 standard-block/hierarchical-I/O/deadline regression corpus, and forged
initializer-store defense regression. No builds, tests, formatter, validators or
mutation discovery were run. Canonical bootstrap was independently verified.

No remaining actionable finding in those inspected changes. Lifetime,
construction, resource-accounting and image fields retain their existing ownership;
callers use the corresponding groups without introducing copies or counter resets.
Dedicated protection/profile/state failures map to distinct stable codes; ordinary
PLC type mismatches remain separate. The actual initializer dispatcher regression
uses a private post-admission mutation, executes LOAD_REF_ADDR/LOAD_CONST/STORE_DEREF,
and expects staging protection while preserving the original global and cleaning
the initializer frame. It is explicitly not proof that malformed wire admission
accepts that body.

The source fixtures use native TOF/TP/R_TRIG/F_TRIG, F_EDGE, CTU/CTD/CTUD, SR/RS,
and declared hierarchical I/O. Reviewed timer/counter/edge expectations match the
existing sampled-delta and edge-state algorithms. The deadline probe's 32nd clock
sample must interrupt actual loop progress and keep the published output unchanged.
These are authored tests, not executed evidence.

Review identified a completion-order issue: polling after engineering mutation or
cycle image publication could return a deadline failure after exposing the change.
The agreed correction defines completion as the final precommit/prepublication
boundary. Engineering reset/entry handling and image staging now follow that rule;
new regressions expire exactly at the last successful run's clock sample and assert
unchanged destinations/outputs. That correction and the shared budget migration
were authored by this reviewer and require the other review agent's independent
review; this note does not self-approve them.

The JSON captures the eight mixed-ownership files inspected. Its hashes identify
bytes, not independent approval of every earlier authored line. Reviewer-owned
assignment, reference traversal, transaction and budget code is excluded from
independent correctness claims. No completion, test-pass, hardware or release claim
is made by this source review.
