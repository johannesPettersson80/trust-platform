#!/usr/bin/env python3
"""Generate the full-software architecture PlantUML map from source-derived facts.

This script intentionally keeps the diagram output generated. Human judgement
belongs in the audit report; the PUML should be a repeatable view over cargo
metadata, line-count maps, module maps, and shallow source scans.
"""

from __future__ import annotations

import json
import os
import re
import subprocess
from collections import Counter, defaultdict
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
GENERATED = ROOT / "docs/internal/architecture/generated"
OUT = ROOT / "docs/diagrams/architecture/full-software-map-generated.puml"


def full_map_artifact_root() -> Path:
    if env_path := os.environ.get("FULL_SOFTWARE_MAP_ARTIFACT"):
        return Path(env_path)
    artifact_dir = ROOT / "target/gate-artifacts"
    candidates = sorted(
        artifact_dir.glob("full-software-map-*"),
        key=lambda path: path.stat().st_mtime,
        reverse=True,
    )
    return candidates[0] if candidates else artifact_dir / "full-software-map-2026-04-28"


ARTIFACT_ROOT = full_map_artifact_root()


def read_json(path: Path) -> dict:
    return json.loads(path.read_text())


def rel(path: Path) -> str:
    return str(path.relative_to(ROOT))


def plantuml_text(text: object) -> str:
    return str(text).replace("\\", "\\\\").replace('"', '\\"')


def identifier(name: str) -> str:
    return re.sub(r"[^A-Za-z0-9_]", "_", name)


def run(args: list[str]) -> str:
    return subprocess.check_output(args, cwd=ROOT, text=True)


def read_line_counts() -> dict[str, dict[str, int]]:
    path = ARTIFACT_ROOT / "static/rust-file-line-counts.tsv"
    counts: dict[str, dict[str, int]] = {}
    if path.exists():
        for line in path.read_text().splitlines():
            crate_path, files, rust_lines, test_files, tests = line.split("\t")
            counts[crate_path] = {
                "files": int(files),
                "rust_lines": int(rust_lines),
                "test_files": int(test_files),
                "tests": int(tests),
            }
        return counts

    software_map = read_json(GENERATED / "software-map.json")
    for package in software_map["packages"]:
        crate_path = package_path(package)
        root = ROOT / crate_path
        rust_files = list(root.rglob("*.rs"))
        test_files = [
            source
            for source in rust_files
            if "tests" in source.relative_to(root).parts or source.name.endswith("_test.rs")
        ]
        counts[crate_path] = {
            "files": len(rust_files),
            "rust_lines": sum(
                len(source.read_text(errors="replace").splitlines()) for source in rust_files
            ),
            "test_files": len(test_files),
            "tests": sum(
                source.read_text(errors="replace").count("#[test]") for source in rust_files
            ),
        }
    return counts


def read_workspace_edges() -> list[tuple[str, str, str]]:
    path = ARTIFACT_ROOT / "static/workspace-direct-deps.tsv"
    if not path.exists():
        software_map = read_json(GENERATED / "software-map.json")
        return [
            (package["name"], dependency, "direct")
            for package in software_map["packages"]
            for dependency in package["trust_dependencies"]
        ]
    return [tuple(line.split("\t")) for line in path.read_text().splitlines() if line.strip()]


def read_hotspots() -> dict[str, int]:
    path = ARTIFACT_ROOT / "static/hotspot-summary.tsv"
    if not path.exists():
        rust_text = "\n".join(
            source.read_text(errors="replace")
            for root in [ROOT / "crates", ROOT / "xtask/src"]
            for source in root.rglob("*.rs")
        )
        return {
            "unsafe_sites": len(re.findall(r"\bunsafe\b", rust_text)),
            "panic_hotspots": len(re.findall(r"\b(?:unwrap|expect)\s*\(|panic!", rust_text)),
            "boundary_hotspots": len(
                re.findall(r"\b(?:boundary|compatibility|deprecated)\b", rust_text)
            ),
        }
    return {
        key: int(value)
        for key, value in (line.split("\t") for line in path.read_text().splitlines())
    }


def read_largest_files(limit: int = 12) -> list[tuple[int, str]]:
    path = ARTIFACT_ROOT / "static/largest-rust-files.txt"
    if not path.exists():
        entries = [
            (len(source.read_text(errors="replace").splitlines()), rel(source))
            for root in [ROOT / "crates", ROOT / "xtask"]
            for source in root.rglob("*.rs")
        ]
        return sorted(entries, reverse=True)[:limit]
    entries: list[tuple[int, str]] = []
    for raw in path.read_text().splitlines():
        parts = raw.split()
        if len(parts) >= 2 and parts[0].isdigit():
            entries.append((int(parts[0]), parts[1]))
    return entries[1 : limit + 1] if entries and entries[0][1] == "total" else entries[:limit]


def rust_line_count(root: Path) -> int:
    total = 0
    if not root.exists():
        return total
    for path in root.rglob("*.rs"):
        total += len(path.read_text(errors="replace").splitlines())
    return total


def file_count(root: Path, pattern: str = "*.rs") -> int:
    if not root.exists():
        return 0
    return sum(1 for _ in root.rglob(pattern))


def read_runtime_modules() -> list[str]:
    lib = ROOT / "crates/trust-runtime/src/lib.rs"
    modules: list[str] = []
    for line in lib.read_text().splitlines():
        match = re.match(r"(?:pub(?:\(crate\))?\s+)?mod\s+([A-Za-z0-9_]+);", line.strip())
        if match:
            modules.append(match.group(1))
        match = re.match(r"pub\s+mod\s+([A-Za-z0-9_]+);", line.strip())
        if match and match.group(1) not in modules:
            modules.append(match.group(1))
    return modules


def runtime_module_files(module: str) -> list[Path]:
    src = ROOT / "crates/trust-runtime/src"
    files: list[Path] = []
    sibling = src / f"{module}.rs"
    if sibling.exists():
        files.append(sibling)
    module_dir = src / module
    if module_dir.exists():
        files.extend(sorted(module_dir.rglob("*.rs")))
    return files


def runtime_module_stats(modules: list[str]) -> dict[str, dict[str, int]]:
    stats: dict[str, dict[str, int]] = {}
    for module in modules:
        files = runtime_module_files(module)
        stats[module] = {
            "files": len(files),
            "lines": sum(len(path.read_text(errors="replace").splitlines()) for path in files),
        }
    return stats


def runtime_module_edges(modules: list[str]) -> Counter[tuple[str, str]]:
    module_set = set(modules)
    edges: Counter[tuple[str, str]] = Counter()
    for module in modules:
        for path in runtime_module_files(module):
            text = path.read_text(errors="replace")
            for target in re.findall(r"\bcrate::([A-Za-z_][A-Za-z0-9_]*)\b", text):
                if target in module_set and target != module:
                    edges[(module, target)] += 1
    return edges


def module_heat_color(lines: int) -> str:
    if lines >= 10_000:
        return "#FFCCBC"
    if lines >= 5_000:
        return "#FFE0B2"
    if lines >= 1_000:
        return "#FFF4D6"
    return "#F5F5F5"


def command_variants() -> list[dict[str, str]]:
    path = ROOT / "crates/trust-runtime/src/bin/trust-runtime/cli/commands.rs"
    lines = path.read_text().splitlines()
    variants: list[dict[str, str]] = []
    doc_lines: list[str] = []
    in_command = False
    for line_no, line in enumerate(lines, start=1):
        if line.strip() == "pub enum Command {":
            in_command = True
            continue
        if in_command and line.startswith("}"):
            break
        stripped = line.strip()
        if not in_command:
            continue
        if stripped.startswith("///"):
            doc_lines.append(stripped.removeprefix("///").strip())
            continue
        match = re.match(r"([A-Z][A-Za-z0-9]+)\s*(?:\{|,)", stripped)
        if match:
            name = match.group(1)
            variants.append(
                {
                    "name": name,
                    "line": str(line_no),
                    "doc": " ".join(doc_lines).strip(),
                    "class": command_class(name),
                }
            )
            doc_lines = []
        elif stripped and not stripped.startswith("#"):
            doc_lines = []
    return variants


def command_class(name: str) -> str:
    runtime = {
        "Run",
        "Play",
        "Ctl",
        "Validate",
        "Build",
        "Hmi",
        "Plcopen",
        "Registry",
        "Setup",
        "Deploy",
        "Rollback",
        "Completions",
        "Bench",
        "Conformance",
    }
    product_ui = {"Ui", "Ide", "ConfigUi", "Wizard"}
    workbench = {"Agent", "Commit", "Docs", "Test"}
    if name in runtime:
        return "runtime/product"
    if name in product_ui:
        return "ui/product"
    if name in workbench:
        return "workbench/dev"
    return "unknown"


def root_surface() -> dict[str, list[str]]:
    tracked = run(["git", "ls-files", "src/main.st", "src/config.st"]).splitlines()
    candidates = [
        "io.toml",
        "runtime.toml",
        "program.stbc",
        "runtime-soak-20260220_031559.log",
        "runtime-soak-20260220_031559.log.runtime.log",
    ]
    present_ignored = [name for name in candidates if (ROOT / name).exists()]
    return {"tracked": tracked, "present_ignored": present_ignored}


def target_counts(packages: list[dict]) -> dict[str, Counter]:
    counts: dict[str, Counter] = {}
    for package in packages:
        counter: Counter = Counter()
        for target in package["targets"]:
            for kind in target["kind"]:
                counter[kind] += 1
        counts[package["name"]] = counter
    return counts


def package_path(package: dict) -> str:
    manifest = Path(package["manifest_path"])
    if manifest.name == "Cargo.toml":
        return rel(manifest.parent)
    return rel(manifest)


def runtime_surface_stats() -> list[tuple[str, int, int]]:
    roots = ["web", "hmi", "ui", "control", "runtime_cloud"]
    result: list[tuple[str, int, int]] = []
    for name in roots:
        module_root = ROOT / f"crates/trust-runtime/src/{name}"
        sibling = ROOT / f"crates/trust-runtime/src/{name}.rs"
        lines = rust_line_count(module_root)
        files = file_count(module_root)
        if sibling.exists():
            files += 1
            lines += len(sibling.read_text(errors="replace").splitlines())
        result.append((name, files, lines))
    return result


def write_diagram() -> None:
    software_map = read_json(GENERATED / "software-map.json")
    packages = sorted(software_map["packages"], key=lambda item: item["name"])
    counts = read_line_counts()
    target_count = target_counts(packages)
    edges = read_workspace_edges()
    hotspots = read_hotspots()
    largest = read_largest_files()
    commands = command_variants()
    runtime_modules = read_runtime_modules()
    runtime_mod_stats = runtime_module_stats(runtime_modules)
    runtime_mod_edges = runtime_module_edges(runtime_modules)
    root = root_surface()
    runtime_surfaces = runtime_surface_stats()

    lines: list[str] = [
        "@startuml full-software-map-generated",
        "' AUTO-GENERATED by scripts/generate_full_software_map_puml.py.",
        "' Source facts: docs/internal/architecture/generated/software-map.json, cargo metadata,",
        "' target/gate-artifacts/full-software-map-2026-04-28, and shallow source scans.",
        "' Do not hand-edit this file; update the generator or source facts instead.",
        "!theme plain",
        "skinparam backgroundColor #FEFEFE",
        "skinparam componentStyle rectangle",
        "skinparam linetype ortho",
        "skinparam nodesep 35",
        "skinparam ranksep 45",
        "skinparam shadowing false",
        "",
        "title truST Full Software Map (Generated From Source Facts)",
        "",
        "legend right",
        "  <b>Generated facts</b>",
        "  Solid arrows: Cargo workspace dependencies",
        "  Orange boxes: source-derived architecture risk hotspots",
        "  Red arrows: current cross-boundary edges needing policy",
        "  Counts come from cargo metadata and line-count artifacts",
        "endlegend",
        "",
    ]

    lines.extend(
        [
            "package \"Product / User Entry Points\" as product #EEF6FF {",
            "  actor \"PLC operator\" as operator",
            "  actor \"Developer\" as developer",
            "  actor \"IDE/LSP client\" as lsp_client",
            "  component \"trust-runtime binary\\n(source: cli/commands.rs)\" as bin_trust_runtime #FFF0D6",
            "  component \"trust-dev binary\\n(package: trust-dev)\" as bin_trust_dev #FFF8E1",
            "  component \"trust-lsp binary\" as bin_trust_lsp #E8F5E9",
            "  component \"trust-debug binary\" as bin_trust_debug #E8F5E9",
            "  component \"trust-harness binary\" as bin_trust_harness #FFF8E1",
            "  component \"trust-bundle-gen binary\" as bin_trust_bundle_gen #FFF8E1",
            "  operator --> bin_trust_runtime",
            "  developer --> bin_trust_runtime",
            "  developer --> bin_trust_dev",
            "  lsp_client --> bin_trust_lsp",
            "}",
            "",
            "package \"Workspace Crates (cargo metadata)\" as workspace #F8F8F8 {",
        ]
    )

    color_by_package = {
        "trust-syntax": "#E8F5E9",
        "trust-hir": "#FFECEC",
        "trust-ide": "#FFF4D6",
        "trust-lsp": "#E9F2FF",
        "trust-dev": "#FFF8E1",
        "trust-plcopen": "#E0F2F1",
        "trust-runtime-core": "#E8F5E9",
        "trust-runtime": "#FFF9C4",
        "trust-debug": "#EDE7F6",
        "trust-wasm-analysis": "#E0F7FA",
        "xtask": "#EEEEEE",
    }
    for package in packages:
        name = package["name"]
        package_id = identifier(name)
        crate_path = package_path(package)
        crate_counts = counts.get(crate_path, {})
        target_summary = target_count[name]
        target_text = ", ".join(f"{kind}:{target_summary[kind]}" for kind in sorted(target_summary))
        lines.append(
            f'  component "{plantuml_text(name)}\\n'
            f'{plantuml_text(crate_path)}\\n'
            f'files:{crate_counts.get("files", "?")} lines:{crate_counts.get("rust_lines", "?")} '
            f'tests:{crate_counts.get("tests", "?")}\\n'
            f'targets:{plantuml_text(target_text)}" as crate_{package_id} {color_by_package.get(name, "#FFFFFF")}'
        )
    lines.append("}")
    lines.append("")

    edge_policy_notes = {
        ("trust-runtime", "trust-ide"): "policy",
        ("trust-lsp", "trust-runtime"): "policy",
        ("trust-debug", "trust-runtime"): "policy",
        ("trust-wasm-analysis", "trust-ide"): "policy",
    }
    for src, dst, _kind in edges:
        src_id = f"crate_{identifier(src)}"
        dst_id = f"crate_{identifier(dst)}"
        if (src, dst) in edge_policy_notes:
            lines.append(f"{src_id} -[#Red,thickness=2]-> {dst_id} : direct dep / needs boundary policy")
        else:
            lines.append(f"{src_id} --> {dst_id} : direct dep")
    lines.append("")

    lines.extend(
        [
            "package \"trust-runtime Current Internal Surfaces\" as runtime_surfaces #FFFDE7 {",
            "  component \"Runtime core\\nruntime/, memory.rs, scheduler, task\" as rt_core #FFF9C4",
            "  component \"Compiler/lowering harness\\nharness/, bundle_builder\" as rt_harness #FFF9C4",
            "  component \"Bytecode VM\\nbytecode/, runtime/vm/\" as rt_vm #FFF9C4",
            "  component \"Legacy/helper eval surface\\neval/, helper_eval/\" as rt_eval #FFF9C4",
            "  component \"Control plane\\ncontrol/\" as rt_control #FFE0B2",
            "  component \"Embedded web/IDE server\\nweb/\" as rt_web #FFCCBC",
            "  component \"HMI contracts + scaffold\\nhmi/\" as rt_hmi #FFECB3",
            "  component \"Terminal UI\\nui/\" as rt_ui #FFECB3",
            "  component \"Runtime cloud\\nruntime_cloud/\" as rt_cloud #FFCCBC",
            "  component \"IO / retain / watchdog / security\" as rt_ops #FFF9C4",
            "  rt_harness --> rt_core",
            "  rt_harness --> rt_vm",
            "  rt_harness --> rt_eval",
            "  rt_core --> rt_ops",
            "  rt_control --> rt_core",
            "  rt_web --> rt_control",
            "  rt_web --> rt_hmi",
            "  rt_web --> rt_cloud",
            "  rt_ui --> rt_control",
            "  rt_cloud --> rt_control",
            "}",
            "",
            "note right of rt_eval",
            "  Source fact: production execution_backend rejects",
            "  'interpreter' config, but eval/helper_eval still exist.",
            "  The ownership strategy should be explicit.",
            "end note",
            "",
            "package \"trust-runtime Top-Level Module Use Graph (generated crate::<module> scan)\" as runtime_module_graph #FAFAFA {",
        ]
    )
    for module in runtime_modules:
        stats = runtime_mod_stats[module]
        lines.append(
            f'  component "{plantuml_text(module)}\\nfiles:{stats["files"]} lines:{stats["lines"]}" '
            f"as rt_mod_{identifier(module)} {module_heat_color(stats['lines'])}"
        )
    for (src, dst), count in sorted(runtime_mod_edges.items()):
        lines.append(f"  rt_mod_{identifier(src)} --> rt_mod_{identifier(dst)} : {count}")
    lines.extend(
        [
            "}",
            "",
            "package \"Runtime Surface Size Overlay (source scan)\" as runtime_size #FFF3E0 {",
        ]
    )
    for name, files, lines_count in runtime_surfaces:
        lines.append(
            f'  component "{plantuml_text(name)}\\nfiles:{files} lines:{lines_count}" '
            f"as rt_size_{identifier(name)}"
        )
    lines.extend(["}", ""])

    lines.extend(
        [
            "package \"trust-runtime CLI Commands (parsed from Command enum)\" as cli_commands #FFF0D6 {",
        ]
    )
    grouped: dict[str, list[dict[str, str]]] = defaultdict(list)
    for command in commands:
        grouped[command["class"]].append(command)
    for group, color in [
        ("runtime/product", "#E8F5E9"),
        ("ui/product", "#FFF4D6"),
        ("workbench/dev", "#FFCCBC"),
        ("unknown", "#EEEEEE"),
    ]:
        if group not in grouped:
            continue
        joined = "\\n".join(
            f'{entry["name"]} (line {entry["line"]})' for entry in grouped[group]
        )
        lines.append(f'  component "{plantuml_text(group)}\\n{plantuml_text(joined)}" as cli_{identifier(group)} {color}')
    lines.extend(
        [
            "}",
            "bin_trust_runtime --> cli_runtime_product",
            "bin_trust_runtime --> cli_ui_product",
            "bin_trust_runtime -[#Red,thickness=2]-> cli_workbench_dev : SRP risk",
            "",
        ]
    )

    lines.extend(
        [
            "package \"Root Working Directory Surface (git/source scan)\" as root_surface #FFF0F0 {",
            f'  component "Tracked root PLC sources\\n{plantuml_text("\\n".join(root["tracked"]) or "none")}" as root_tracked',
            f'  component "Present ignored runtime artifacts\\n{plantuml_text("\\n".join(root["present_ignored"]) or "none")}" as root_ignored',
            "}",
            "root_tracked -[#Orange]-> bin_trust_runtime : repo-root project fixture",
            "root_ignored -[#Orange,dashed]-> bin_trust_runtime : local runtime working-dir coupling",
            "",
        ]
    )

    lines.extend(
        [
            "package \"Architecture Risk Overlays (generated counts)\" as risks #FFECEC {",
            f'  component "Hotspots\\nunsafe occurrences:{hotspots.get("unsafe_sites", 0)}\\n'
            f'panic/unwrap/etc:{hotspots.get("panic_hotspots", 0)}\\n'
            f'boundary markers:{hotspots.get("boundary_hotspots", 0)}" as risk_hotspots #FFCDD2',
            f'  component "Architecture doctor\\nxtask/src/main.rs:{rust_line_count(ROOT / "xtask/src")} lines\\n'
            f'single binary automation surface" as risk_doctor #FFCDD2',
            f'  component "Runtime modules\\n{len(runtime_modules)} top-level modules" as risk_runtime_modules #FFCDD2',
            "  component \"Largest files\" as risk_largest #FFCDD2",
        ]
    )
    for idx, (line_count, path) in enumerate(largest[:8], start=1):
        lines.append(f'  component "{line_count} lines\\n{plantuml_text(path)}" as largest_{idx} #FFECB3')
        lines.append(f"  risk_largest --> largest_{idx}")
    lines.extend(
        [
            "}",
            "risk_runtime_modules -[#Red]-> crate_trust_runtime : KISS risk",
            "risk_doctor -[#Orange]-> crate_xtask : monolithic doctor",
            "risk_hotspots -[#Orange]-> workspace : triage map",
            "",
            "package \"Test Architecture Surface\" as test_surface #EEF6FF {",
        ]
    )
    runtime_package = next(package for package in packages if package["name"] == "trust-runtime")
    test_targets = [
        target["name"]
        for target in runtime_package["targets"]
        if "test" in target["kind"]
    ]
    part_files = sorted(
        rel(path)
        for path in (ROOT / "crates/trust-runtime/tests").rglob("*.rs")
        if "_part_" in path.name
    )
    lines.append(f'  component "trust-runtime integration targets\\n{len(test_targets)} cargo test targets" as test_runtime')
    lines.append(f'  component "part-split test files\\n{len(part_files)} files" as test_parts')
    for idx, path in enumerate(part_files[:8], start=1):
        lines.append(f'  component "{plantuml_text(path)}" as test_part_{idx} #E3F2FD')
        lines.append(f"  test_parts --> test_part_{idx}")
    if len(part_files) > 8:
        lines.append(f'  component "... {len(part_files) - 8} more" as test_part_more #E3F2FD')
        lines.append("  test_parts --> test_part_more")
    lines.extend(
        [
            "}",
            "test_runtime -[#Orange]-> crate_trust_runtime : compile/test surface",
            "test_parts -[#Orange]-> crate_trust_runtime : KISS test-structure risk",
            "",
            "package \"Automated Evidence Inputs\" as evidence #F5F5F5 {",
            "  artifact \"software-map.json\" as ev_software_map",
            "  artifact \"cargo metadata\" as ev_cargo_metadata",
            "  artifact \"workspace-direct-deps.tsv\" as ev_edges",
            "  artifact \"rust-file-line-counts.tsv\" as ev_line_counts",
            "  artifact \"largest-rust-files.txt\" as ev_largest",
            "  artifact \"hotspot-summary.tsv\" as ev_hotspots",
            "  artifact \"mutants-hir-summary.json\" as ev_mutants_hir",
            "  artifact \"cargo-audit.json\" as ev_audit",
            "}",
            "ev_software_map ..> workspace",
            "ev_cargo_metadata ..> workspace",
            "ev_edges ..> workspace",
            "ev_line_counts ..> runtime_size",
            "ev_largest ..> risks",
            "ev_hotspots ..> risks",
            "ev_mutants_hir ..> crate_trust_hir : semantic-test adequacy finding",
            "ev_audit ..> risks : supply-chain finding",
            "",
            "note bottom",
            "  This PUML is generated. It maps source facts and rule-derived risk overlays.\\n",
            "  It does not replace the audit report; it makes the current architecture visible.\\n",
            "  To update: run scripts/generate_full_software_map_puml.py, then refresh diagram manifest.",
            "end note",
            "@enduml",
            "",
        ]
    )

    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text("\n".join(lines))


if __name__ == "__main__":
    write_diagram()
