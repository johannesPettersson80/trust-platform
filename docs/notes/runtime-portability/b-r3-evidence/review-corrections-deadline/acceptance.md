# Independent deadline correction review

Root reviewed the numeric agent's four-file correction against the frozen M2 source.
Accepted by source inspection: one operation-scoped Cell latches expiry; reset clears
it with fuel and stride. Engine-only deadline sampling wraps the service; policy
entry/work checks reuse ExecutionContext defaults, reaching the same latch from
dispatch, native calls and terminal cleanup. Hosted Runtime sampling is unchanged.
Cleanup still consumes fuel and retires identities; original errors remain preserved.
The new retirement regression crosses a real stride and checks frame/instance removal,
no resampling, and reset rearming. The original hosted exact-32-poll assertion remains.
No tests, compiler, formatter or new link ran. New firmware size and behavior are unverified.
