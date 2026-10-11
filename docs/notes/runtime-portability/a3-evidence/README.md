# A3 validation evidence

A3 review corrections are scope-verified through runs 5 and 6. The work is
locally committed (`7e6938a75` implementation) and is not release-qualified or
executable by the hosted 2.0 engine.

- Run 5: 224 core tests, 70 portable-loader tests, 142 i686 musl tests, both MCU
  library checks, 219 hosted integrations and 3,863 runtime unit tests pass.
  Clippy, Linux/Windows warnings, architecture, canonical diagrams/drift, formatting
  and advisory metadata pass. The three source-authoring fixture failures remain
  in the original ledger; they are not erased by later passing evidence.
- Run 6: all 31 source-authoring tests and four authoring-boundary unit tests pass,
  including the fixture corrections and lowered partial-configuration regression.
  All eight required and two advisory steps pass. Only two Rust test files changed
  after run 5; production source and generated fixture bytes retain run-5 evidence.
- Run 1 dependency/no-dev graph evidence remains applicable because Cargo manifests
  and lockfile are unchanged. Runs 1–4 preserve their historical evidence and limits.

The 6,164-byte fixture includes an edge input and retained activation state. Its
SHA-256 is `0a09a7190f26bbe3481ca7ae5b7f16a130ae4dcb2ee21937b456dd8581a87e5a`.
It was generated, decoded, admitted and compared with fresh production. No 2.0
initializer has executed in the shared source-free engine, and no firmware has
linked or run on a board. A4 and the hardware scopes own that evidence.

`artifact-sha256.json` indexes the retained commands, raw `.txt` outputs, ledgers,
manifests, review reports, exact current review input archives and this README.
The index itself is excluded to avoid self-reference. Historical review digests
are identified as historical where original pre-format inputs were not retained;
current review identities are reproducible from their archived bytes.

Closeout-only document edits follow the frozen manifests. They do not change Rust
or the generated artifact and do not claim an additional validation run.
