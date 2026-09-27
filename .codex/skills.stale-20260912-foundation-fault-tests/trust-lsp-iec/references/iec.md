# IEC 61131-3 References (ST)

Authoritative source:
- `docs/internal/standards/IEC 61131-3_2013 Ed3.pdf` when present

Searchable text:
- `docs/internal/standards/iec61131-3.txt` when present (use `rg -n` to locate tables/sections quickly)

Fallback:
- If both IEC files are absent locally and `/home/johannes/Downloads/iec-61131-3.ocr.txt` exists, copy it to `docs/internal/standards/iec61131-3.txt` before IEC proof work.
- If no IEC source is available locally, state the gap and verify on `trust-builder` or with an explicitly provided standard source before claiming IEC proof.

Specs mapping:
- `docs/specs/README.md`
- `docs/specs/*.md`

Decision logs:
- Ambiguity: `docs/IEC_DECISIONS.md`
- Normative IEC conflict, omission, or relaxation only: `docs/IEC_DEVIATIONS.md`
- IEC-silent, out-of-scope, implementation-specific, platform/runtime, or
  truST-only API behavior: relevant `docs/specs/*.md` file, not an IEC log

Deviation entry precondition:
1. Cite the exact IEC section/table.
2. State the normative IEC requirement.
3. State truST's behavior.
4. Explain the concrete conflict. If there is no conflict, do not add an entry.

Quick extraction (PDF):
```
pdftotext -f <page> -l <page> "docs/internal/standards/IEC 61131-3_2013 Ed3.pdf" - | sed -n '1,200p'
```

Key ST tables/sections:
- Tables 1-9: lexical elements and literals
- Table 10 + Figure 5: elementary types and generic hierarchy
- Tables 13-16: variables, direct addressing, arrays
- Tables 19, 40, 47, 48, 51: FUNCTION, FUNCTION_BLOCK, PROGRAM, CLASS, INTERFACE
- Table 71: operators and precedence
- Table 72: statements
- Tables 22-36: standard functions and conversions
- Tables 43-46: standard function blocks
