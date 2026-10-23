# Independent source review: deadline correction

Accepted by collections agent; no blocking finding. Compared all four files against the retained B-R3 M2 core snapshot. No tests, builds, formatter, link, or hardware execution performed. This is source review only; correction remains unvalidated.

The latch is owned by the shared ExecutionBudget and cleared only by reset, preserving operation and nested-call boundaries. Portable ExecutionContext deadline sampling uses this latch; the hosted context is unchanged. Entry and charged-work policy now delegate to existing trait helpers, preserving fuel accounting and stride. Terminal frame retirement still performs all cleanup even when its work charge returns the latched deadline error. It does not re-observe a clock that already expired.

Native additions retain existing assertions and cover pre-expiry samples, sticky expiry even if the service disarms, remaining fuel and stride, real owned-instance/frame cleanup, nested entry without reset, and the next explicit operation reset. Imports and trait-method calls resolve by source inspection. The original integration assertion of 32 polls is unchanged.

Identity: source-identity.json records complete current file SHA-256 digests, before final formatting. No claims about compilation or exact firmware size.
