# H manual authoring seed projection

Status: source implementation and native assertions authored; no validation evidence.

Specification 34 HOST-09 confines unrestricted storage mutation to non-executable
construction. `RuntimeBuilder::build` consumes that construction context and uses
the same declaration producer, STBC validation, preparation and installed owner as
source compilation. It never reparses synthetic Structured Text or restores an old
executable Runtime.

Registered global/program/FB/class declarations define the admissible schema.
Seed values may override declared initial values. They do not add undeclared
variables, executable types or runtime frames. Every stored root and owned instance
must map to a declared symbolic path. Missing or stale references, cycles in owned
instance graphs, shared ownership of an instance by distinct declared roots, unknown
members and mismatched declared types are rejected before installation. Ordinary
reference-valued aliases can share a declared persistent referent; their raw owner
IDs are translated to symbolic paths before the artifact assigns new identities.

Arrays, structures and unions retain their declared dimensions/member identities.
Scalar/value defaults become typed initializer expressions. Raw instance graphs
must become typed staged construction, including inherited fields; they cannot be
copied into the installed store. Frame-local and execution-frame references are not
construction inputs. Native hidden execution state is not an authored declaration.

Engineering aliases and I/O bindings are translated through the same symbolic
binding map. No incoming offset or instance ID becomes an installed identity by
casting. Initial image bytes and hierarchical values need explicit image
initialization records; accepting bindings must not silently discard their values.
Task FB bindings undergo the same translation. The final prepared artifact and
checked runtime construction remain the authority on capacities and legality.

Source-language construction rejects CLASS initializers and the ordinary FB member
initializer helper resolves only own members. Manual declared instance seeds now
use a builder-only staged lowering path through DEFAULT_TYPED and private
initializer-result references, including inherited constant members. A flat
configuration write is not equivalent and is not used to bypass constant-write
protection. Source-language restrictions remain unchanged; the common dispatcher
executes the resulting initialization. This is authored source, not executed proof.

Native evidence must cover scalar and aggregate seeds, declared FB/class/program
instances and inherited fields, persistent references, aliases and I/O, task bindings,
and rejection without a partially installed controller. No test has run for this
work yet.
