# ADS management write ownership

Source-only H cleanup; native regressions are authored and unrun.

The old DAP global write published the PLC value, attempted ADS queueing, and
restored the value if queueing failed. The first H translation made restoration
fallible by using a checked owner write. Full source tracing established that the
queue-failure branch was unreachable after a positive writable qualification:
the owner holds an exclusive Runtime borrow, both predicates inspect the same
immutable binding name/access, per-bridge duplicate names are forbidden, and the
shared cache queue operation has no rejection result. No pre-fix failure is
claimed.

The installed owner now performs `write_global_and_queue_ads`: select the first
writable bridge before mutation (or reject a read-only point), perform the common
checked value write, capture the canonical admitted value (result cloning is charged before commit),
then publish through that selected bridge's crate-private
qualified queue helper. A non-ADS global takes only the checked write. This keeps
existing first-match and generation behavior, removes the unreachable rollback,
and prevents a future caller from separating qualification from the owned write.
The DAP caller retains its existing read-only explanation and force-after-success
ordering.

Native source assertions cover invalid type and expired deadline before queue,
read-only rejection without effects, ordinary globals, first writable connection
only, one generation increment per accepted queue command, and INT-to-DINT/REAL/alias
normalization through the actual ADS wire encoder. They do not claim
wire/device acceptance; asynchronous transport acknowledgements remain unchanged.
