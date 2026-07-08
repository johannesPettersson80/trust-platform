# Conformance Naming Rules

The conformance suite requires deterministic naming for cases, expected
artifacts, and summary reports.

## Category Names

Valid v1 category directory names:

- `timers`
- `edges`
- `scan_cycle`
- `init_reset`
- `arithmetic`
- `memory_map`

Valid v2 category directory names also include:

- `strings`
- `arrays`
- `structs`
- `enums`
- `nested_values`
- `oop_dispatch`
- `references`
- `retain_matrix`
- `scheduler`
- `comms_determinism`

## Case ID Format

Case IDs are lowercase and encode category + behavior + sequence:

```text
cfm_<category>_<topic>_<scenario>_<nnn>
```

Rules:

- Prefix is always `cfm_`
- `<category>` must be one frozen category
- `<topic>` and `<scenario>` are lowercase `[a-z0-9]+` tokens separated by `_`
- `<nnn>` is a zero-padded 3-digit sequence per category/topic lane

Regex:

```text
^cfm_(timers|edges|scan_cycle|init_reset|arithmetic|memory_map|strings|arrays|structs|enums|nested_values|oop_dispatch|references|retain_matrix|scheduler|comms_determinism)_[a-z0-9]+(?:_[a-z0-9]+)*_[0-9]{3}$
```

## Case Folder and Files

Case folder path:

```text
conformance/cases/<category>/<case_id>/
```

Required files inside each case folder:

- `program.st`
- `manifest.toml`

## Expected Artifact Naming

Expected outputs are stored separately from source cases:

```text
conformance/expected/<category>/<case_id>.json
```

## Summary Report Naming

Generated summary reports go to:

```text
target/conformance/<timestamp>_<runner>_summary.json
```

Timestamp format is UTC basic ISO-like:

```text
YYYYMMDDTHHMMSSZ
```

Example:

```text
20260210T120000Z_trust-runtime_summary.json
```

`conformance/reports/` is reserved for local ignored/generated output and keeps
only `.gitkeep` in Git.
