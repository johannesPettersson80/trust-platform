# Phase 17 code-to-spec baseline freeze

Status: implementation evidence. This is not the independent
`VERIF-P17-006` or `VERIF-P17-012` verdict.

## Frozen revisions

- canonical repository: `/home/johannes/projects/trust-platform`
- branch: `plc-verification-program`
- implementation SHA: `1aacc2094e66cc65019eddea14889f78269a2e05`
- immediate-child baseline SHA:
  `5ecc4626b0305d2b6d833c82f7fe9be29dc13bbe`
- baseline file SHA-256:
  `76345fdaaa82c56528baeef2432fe57d860602710fdfd9c25172399d81dbeca3`
- detached reproduction checkout:
  `/home/johannes/projects/trust-platform-p17-freeze-1aacc209`

`git rev-list --parents -n 1 5ecc4626b0305d2b6d833c82f7fe9be29dc13bbe`
returned exactly the baseline SHA followed by the implementation SHA. The
baseline declares the same implementation SHA in `created_from_commit`, uses
`enforcement_mode = "changed"`, and uses `append_policy = "delete_only"`.

## Bootstrap and private input

The canonical source was `/home/johannes/projects/trust-platform`. Before the
scan, the root `AGENTS.md`, complete `.codex/skills/**` tree, and private hook
were copied manually into the detached destination and compared byte-for-byte.

- destination `AGENTS.md` SHA-256:
  `2808d20386ee2604bd20136f4b9b767686a491ba27f981d5cc94f1f6ed4f3be3`
- destination relative-path `.codex/skills/**` aggregate SHA-256:
  `fda1aaf8ceed69e8208f75fdc6484328ac8d63fea846de1f8f7f618cdd66ab68`
- `.claude/hooks/pre_tool_use_gate.py` SHA-256:
  `be49797cabae8807d39c41e3c0d75b1dd527ada4e2c8889120aab6d7f7ec911d`

The TypeScript adapter used the lockfile-pinned compiler API version `5.9.3`;
the installed package version and `editors/vscode/package-lock.json` entry both
reported `5.9.3`.

## Exact partition

The clean scan produced:

- 15,228 live facts;
- 93 exact mappings;
- 15,135 unique frozen debt entries;
- 0 live facts absent from both the map and baseline;
- 32 risk acknowledgement references and 32 ledger records;
- 2,177 still-visible unresolved area/owner facts inside historical debt.

The source census was 1,059 Rust product files, 196 TypeScript/JavaScript
product files plus 2 packaging inputs, 258 Python verification-tooling files,
23 Rust verification-tooling files, and 81 shell verification-tooling files.
The baseline is intentionally not a semantic acceptance of those debt rows.

## Canonical report and changed ratchet

From the clean baseline checkout, with the report root explicitly bound to that
checkout:

```text
python3 scripts/code_spec_coverage_gate.py --mode report \
  --out-dir target/gate-artifacts/p17-baseline/report
code-spec coverage generated: 15228 facts; 2177 findings; 0 blocking in report mode

python3 scripts/code_spec_coverage_gate.py --mode changed \
  --base 1aacc2094e66cc65019eddea14889f78269a2e05 \
  --head 5ecc4626b0305d2b6d833c82f7fe9be29dc13bbe \
  --out-dir target/gate-artifacts/p17-baseline/changed
code-spec coverage generated: 15228 facts; 2177 findings; 0 blocking in changed mode
```

Both commands exited 0 and their JSON/Markdown pairs passed the production
at-rest validator. Report-mode artifact digests were:

- JSON:
  `36484c1ecf25a4232f733a45f1a9fc1ecfb3a5f161becd56aa1d664753f6f1da`
- Markdown:
  `b22e130b8d02687a047bf80f970d805a46066a861fec99671a7135f2414415de`

Changed-mode artifact digests were:

- JSON:
  `d23da844416553c73e9bf8be025890f55d5275f8e0882eb276aaefa017d7fd52`
- Markdown:
  `2d9439dccb644be25f5af3e12d63f8d74db3624974847aa243377795026074a5`

The report input-closure digest was
`sha256:c336d8ce158fc3770605d90275cc19ad7dcf414e17a561c09086ae153dc37807`.
This was local Linux AArch64 proof. The final remote-builder
`fmt`/`clippy`/`test-all` milestone remains deliberately deferred until the
Phase 17 debt reaches zero and the final review SHA is frozen.
