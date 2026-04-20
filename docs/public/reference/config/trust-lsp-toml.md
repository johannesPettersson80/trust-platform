# `trust-lsp.toml`

`trust-lsp.toml` configures workspace indexing, reusable libraries,
dependencies, diagnostics, vendor profile behavior, and runtime-assisted editor
features.

The language server discovers these filenames:

- `trust-lsp.toml`
- `.trust-lsp.toml`
- `trustlsp.toml`

## Minimal Example

```toml
[project]
include_paths = ["src"]
vendor_profile = "siemens"
stdlib = "iec"

[runtime]
control_endpoint = "unix:///tmp/trust-runtime.sock"
```

## `[project]`

| Key | Type | Default | Notes |
| --- | --- | --- | --- |
| `include_paths` | string array | `[]` | Additional project source roots to index. |
| `library_paths` | string array | `[]` | Legacy/index-only library roots. |
| `vendor_profile` | string | none | Controls formatting and diagnostic defaults. |
| `stdlib` | string or string array | `full` | String = profile; array = allowlist of names. |

Common `stdlib` forms:

| Form | Meaning | Example |
| --- | --- | --- |
| string profile | use a named built-in profile | `stdlib = "full"` |
| IEC-only profile | limit to IEC-facing standard library | `stdlib = "iec"` |
| disabled | no preloaded stdlib symbols | `stdlib = "none"` |
| explicit allowlist | load only named entries | `stdlib = ["ABS", "CTU", "TON"]` |

## `[dependencies]`

Use this for reusable truST packages that should participate in normal project
compilation.

| Key | Type | Required | Notes |
| --- | --- | --- | --- |
| `<name>.path` | string | path dependency | Local package root. |
| `<name>.git` | string | git dependency | Remote git URL. |
| `<name>.version` | string | recommended | Resolver/version contract. |
| `<name>.rev` / `tag` / `branch` | string | optional | Choose at most one git locator. |

Path dependency:

```toml
[dependencies]
MyLib = { path = "../libraries/my_lib", version = "0.1.0" }
```

Git dependency:

```toml
[dependencies]
MyLib = { git = "https://example.com/my-lib.git", tag = "v0.1.0", version = "0.1.0" }
```

Dependency rules:

| Condition | Requirement |
| --- | --- |
| dependency source | set exactly one of `path` or `git` |
| git locator | set at most one of `rev`, `tag`, or `branch` |
| locked/offline behavior | configure from `[build]` |

## `[[libraries]]`

Use this for index-only packs, vendor stubs, or attached docs that should not
be treated as first-class project dependencies.

```toml
[[libraries]]
name = "siemens-stubs"
path = "vendor/siemens"
version = "0.1.0"
docs = ["docs/vendor.md"]
```

| Key | Type | Required | Notes |
| --- | --- | --- | --- |
| `name` | string | yes | Display/name key. |
| `path` | string | yes | Library root. |
| `version` | string | no | Informational or resolver hint. |
| `dependencies` | array of tables | no | Index-only dependency metadata. |
| `docs` | string array | no | Attached docs paths. |

## `[build]`

| Key | Type | Default | Notes |
| --- | --- | --- | --- |
| `target` | string | none | Editor-side build target hint. |
| `profile` | string | none | Profile name such as `debug` or `release`. |
| `flags` | string array | `[]` | Additional compile flags. |
| `defines` | string array | `[]` | Preprocessor/define flags. |
| `dependencies_offline` | bool | `false` | Disables network fetch/clone for git deps. |
| `dependencies_locked` | bool | `false` | Requires pinned revisions or lock entries. |
| `dependency_lockfile` | string | `trust-lsp.lock` | Lock file path. |

`[[targets]]` can override `profile`, `flags`, and `defines` per named target.

## `[indexing]`

| Key | Type | Default |
| --- | --- | --- |
| `max_files` | integer | none |
| `max_ms` | integer | none |
| `cache` | bool | `true` |
| `cache_dir` | string | `.trust-lsp/index-cache` |
| `memory_budget_mb` | integer | none |
| `evict_to_percent` | integer | `80` |
| `throttle_idle_ms` | integer | `0` |
| `throttle_active_ms` | integer | `8` |
| `throttle_max_ms` | integer | `50` |
| `throttle_active_window_ms` | integer | `250` |

## `[diagnostics]`

| Key | Type | Default | Notes |
| --- | --- | --- | --- |
| `warn_unused` | bool | profile default | Toggle unused diagnostics. |
| `warn_unreachable` | bool | profile default | Toggle unreachable-code warnings. |
| `warn_missing_else` | bool | profile default | Toggle missing-ELSE warnings. |
| `warn_implicit_conversion` | bool | profile default | Toggle implicit-conversion warnings. |
| `warn_shadowed` | bool | profile default | Toggle shadowing warnings. |
| `warn_deprecated` | bool | profile default | Toggle deprecation warnings. |
| `warn_complexity` | bool | profile default | Toggle complexity warnings. |
| `warn_nondeterminism` | bool | profile default | Toggle nondeterminism warnings. |
| `warn_numeric_hazards` | bool | profile default | Toggle numeric-hazard warnings. |
| `rule_pack` | string | none | Named diagnostic bundle. |
| `external_paths` | string array | `[]` | Extra paths to analyze. |
| `severity_overrides` | table | `{}` | Per-code overrides such as `W003 = "error"`. |

Supported rule packs include:

- `iec-safety`
- `safety`
- `siemens-safety`
- `codesys-safety`
- `beckhoff-safety`
- `twincat-safety`
- `mitsubishi-safety`
- `gxworks3-safety`

Vendor profiles also affect defaults:

- `siemens`: disables missing-ELSE and implicit-conversion warnings by default
- `codesys` / `beckhoff` / `twincat`: keep standard warning set enabled
- `mitsubishi` / `gxworks3`: keep standard warning set enabled

## `[runtime]`

| Key | Type | Notes |
| --- | --- | --- |
| `control_endpoint` | string | Runtime control endpoint for debug-assisted features. |
| `control_auth_token` | string | Optional auth token for the control endpoint. |

## `[workspace]`

| Key | Type | Default | Notes |
| --- | --- | --- | --- |
| `priority` | integer | `0` | Workspace federation priority. |
| `visibility` | string | `public` | `public`, `private`, or `hidden`. |

## `[telemetry]`

| Key | Type | Default | Notes |
| --- | --- | --- | --- |
| `enabled` | bool | `false` | Opt-in only. |
| `path` | string | `.trust-lsp/telemetry.jsonl` when enabled | Output file. |
| `flush_every` | integer | `25` | Flush interval. |

## `[dependency_policy]`

Use this to constrain git dependencies:

```toml
[dependency_policy]
allowed_git_hosts = ["github.com", "gitlab.com"]
allow_http = false
allow_ssh = false
```

| Key | Type | Default | Notes |
| --- | --- | --- | --- |
| `allowed_git_hosts` | string array | `[]` | Explicit host allowlist. |
| `allow_http` | bool | `false` | Permit insecure git/http transport. |
| `allow_ssh` | bool | `false` | Permit SSH transport. |

## Optional `[package]`

Reusable library manifests may also declare:

```toml
[package]
version = "0.1.0"
```

That version is used by the dependency resolver when another project consumes
the package.

| Key | Type | Required | Notes |
| --- | --- | --- | --- |
| `version` | string | yes | Package version exposed to dependents. |

## Related

- [Vendor Profiles](../../develop/vendor-profiles.md)
- [Project Layout](../../develop/project-layout.md)
- [Agent Quickstart](../../start/agent-quickstart.md)
