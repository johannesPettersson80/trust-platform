# Hosted resource-only reconfiguration

Source-only H correction; no validation has run for this slice.

At the H base, `runtime/bytecode.rs::apply_resource_metadata` validated task
associations, resized images, cleared task readiness and re-registered tasks.
Registration sampled current SINGLE values and retained debugger thread IDs.
It never restarted PLC values, reset cycle/register counters, cleared faults or
cleared debugger requests. The `bytecode_metadata::pou_associations` assertion
sets a live FB input before applying resource metadata and relies on that input
and its instance identity afterwards. Treating this operation as warm replacement
was an H integration regression, not a required behavior change.

Spec 34 HOST-09 now separates resource-only configuration from code/type/layout
replacement. The shared owner prepares an exclusively borrowed resource token:
exact executable/construction identities must match, task/image allocation is
charged before growth, and only flat images and scheduler scratch are staged.
Flat image intersections survive; growth is zeroed. SINGLE is sampled when task
metadata is registered. Binding-only updates preserve task readiness. There is no
PLC object clone, initializer execution or graph reconstruction. The old object
IDs, hierarchical images, logical time, fault and register state remain owned by
the same controller.

Host schema assembly borrows that unchanged owner. Direct harness binding and
MQTT batch mapping both produce an admitted resource/I/O-map candidate. Codecs
come from the unchanged reference TYPE_TABLE identity, not an untyped host
mutation. ADS/OPC UA workers and pending bindings retain their exact destinations,
because types, access and reference identities cannot change in this operation.
Safe-output policy is validated against staged image extents. Commit checks the
physical deadline before swapping resource/images; host publication has no later
fallible work. Dropping or rejecting a candidate preserves images, readiness,
artifact, PLC values and management queues.

Native regressions are authored in core `resource_reconfiguration` and hosted
`installed_lifecycle/tests`: owner/value/fault preservation, dropped candidate,
image intersection/growth, changed execution/profile rejection, live FB input and
object address preservation, queued force, SINGLE baseline, stable thread IDs,
binding-only timing and malformed task/binding rollback. Existing boundary and
MQTT mapping suites retain their assertions and now exercise the admitted path.
