# A4 run-4 reference-type correction source review

No actionable source finding in the seven paths pinned by the [manifest](run-4-reference-types-review.json).

The new helper resolves global/frame/instance declarations, then traverses POU
members through physical construction metadata and inherited templates. It excludes
External entries both at base-slot and member lookup. Untyped ProgramRoot paths
start from the declared program POU; whole untyped roots still have no fabricated
TYPE_TABLE identity. Other segments retain the existing alias/array/structure/string
path policy. Readonly checks reuse physical member lookup and retain constant gates.

All reference_type callers propagate Result errors. Exhausted work/deadline errors
remain operational errors, not successful missing-type fallback or TypeMismatch.
RetainedGraph queries the mapped candidate instance after reserve_instance installs
its template/physical fields, so typing spends the candidate restart budget rather
than the source state's exhausted prior scan fuel. The new native assertions use
actual saved Counter.value INT3 and Plant.activations DINT metadata.

The inherited-member source fixture uses a public ancestor field and a persistent
global reference target. The External-shadow fixture clones an Instance declaration
whose ref_idx is None. Existing validation permits that binding absence for instance
records, excludes External names/slots from owned uniqueness and demand, and skips
External entries in the physical declaration index. Consequently changing only its
non-owning type to BOOL does not conflict with physical INT VAR_META or references.
The physical declaration/root/initializer identities are shifted consistently after
insertion. Preparation remains a positive assertion; the wrong real BOOL reference
is separately rejected without replacing the previous INT reference. No admission
rule or source diagnostic was relaxed. Fixture construction and assertions are split
into separate functions. The original failing restart regression remains unchanged.

This is source inspection, not compilation or runtime proof. No tests, builds,
formatters, validators or batch commands ran. Earlier code authored by this reviewer
is not independently approved by this correction review. Root's architecture-test
and run-5 script review is retained separately in run-4-architecture-batch-review.json.

Aggregate SHA256: `31e1281c11ef7e31edb54d36ff3392b0d8fd4f93cdc67ebe16cff8c48000bda9`.
