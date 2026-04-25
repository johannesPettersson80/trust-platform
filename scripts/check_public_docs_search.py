#!/usr/bin/env python3

from __future__ import annotations

import json
import re
import sys
from pathlib import Path


SEARCH_INDEX = Path("site/public/search/search_index.json")

QUERY_EXPECTATIONS: list[tuple[str, str]] = [
    ("install", "start/installation/"),
    ("install windows", "start/installation/"),
    ("install mac", "start/installation/"),
    ("first project", "start/first-project/"),
    ("agent quickstart", "start/agent-quickstart/"),
    ("agent serve", "start/agent-quickstart/"),
    ("vscode", "start/program-in-vscode/"),
    ("vs code", "start/program-in-vscode/"),
    ("hmi", "operate/hmi-and-web-ui/"),
    ("ladder", "develop/visual-editors/ladder/"),
    ("visual editor", "develop/visual-editors/"),
    ("companion st", "develop/visual-editors/companion-st/"),
    ("codesys", "migrate/codesys-twincat/"),
    ("connect runtimes", "connect/runtime-to-runtime/"),
    ("protocols", "connect/protocol-matrix/"),
    ("ethercat", "connect/devices-and-fieldbus/ethercat/"),
    ("mqtt", "connect/external-systems/mqtt/"),
    ("modbus tcp", "connect/external-systems/modbus-tcp/"),
    ("opc ua", "connect/external-systems/opc-ua/"),
    ("runtime toml", "reference/config/runtime-toml/"),
    ("e001", "reference/diagnostics/"),
    ("w003", "reference/diagnostics/"),
    ("w005 implicit conversion", "reference/diagnostics/"),
    ("w004 missing else", "reference/diagnostics/"),
    ("deploy", "operate/deploy-rollback/"),
    ("browser ide", "start/editors/"),
    ("neovim", "start/editors/"),
    ("statechart", "develop/visual-editors/statechart/"),
    ("blockly", "develop/visual-editors/blockly/"),
    ("sfc", "develop/visual-editors/sfc/"),
    ("twincat", "migrate/codesys-twincat/"),
    ("siemens", "migrate/siemens/"),
    ("siemens import", "migrate/siemens/"),
    ("mitsubishi", "migrate/mitsubishi/"),
    ("plcopen", "migrate/plcopen/"),
    ("third party st", "migrate/"),
    ("package registry", "develop/package-registry/"),
    ("project layout", "develop/project-layout/"),
    ("vendor profile", "develop/vendor-profiles/"),
    ("io toml", "reference/config/io-toml/"),
    ("%ix0.0", "connect/devices-and-fieldbus/io-binding/"),
    ("simulation toml", "reference/config/simulation-toml/"),
    ("trust lsp toml", "reference/config/trust-lsp-toml/"),
    ("ctu timer", "reference/specifications/08-standard-function-blocks/"),
    ("structured text new project", "start/create-new-project/"),
    ("pid loop", "examples/tutorials/"),
    ("raspberry pi", "operate/install-on-target/"),
    ("watchdog", "reference/config/runtime-toml/"),
    ("watchdog timeout", "reference/config/runtime-toml/"),
    ("fault policy", "reference/config/runtime-toml/"),
    ("retain", "reference/config/runtime-toml/"),
    ("runtime to runtime transports", "connect/runtime-to-runtime/transport-matrix/"),
    ("remote access", "connect/networking-and-remote-access/"),
    ("simulated", "connect/devices-and-fieldbus/simulated-and-loopback/"),
    ("loopback", "connect/devices-and-fieldbus/simulated-and-loopback/"),
    ("safe state", "operate/safety-and-commissioning/"),
    ("e-stop", "operate/safety-and-commissioning/"),
    ("zenoh", "connect/runtime-to-runtime/mesh-zenoh/"),
    ("mesh", "connect/runtime-to-runtime/mesh-zenoh/"),
    ("discovery", "connect/runtime-to-runtime/discovery-and-pairing/"),
    ("pairing", "connect/runtime-to-runtime/discovery-and-pairing/"),
    ("scan cycle", "concepts/scan-cycle/"),
    ("build validate test", "operate/build-validate-test/"),
    ("test json output", "operate/build-validate-test/"),
    ("test junit", "operate/build-validate-test/"),
    ("debug", "operate/debugging-and-runtime-panel/"),
    ("trust-debug", "reference/cli/trust-debug/"),
    ("runtime panel", "operate/debugging-and-runtime-panel/"),
    ("control endpoint", "operate/runtime-ui-and-control/"),
    ("hot reload", "operate/compile-validate-reload/"),
    ("rollback", "operate/deploy-rollback/"),
    ("observability", "operate/observability/"),
    ("metrics", "operate/observability/"),
    ("historian", "operate/observability/"),
    ("conformance", "reference/conformance/"),
    ("benchmarks", "reference/benchmarks/"),
    ("harness", "concepts/deterministic-harness/"),
    ("harness protocol", "reference/harness/protocol/"),
    ("alarm acknowledge", "operate/operator-alarm-handbook/"),
    ("agent serve stdio", "start/automate-with-cli/"),
    ("trust-runtime flags", "reference/cli/trust-runtime/"),
]


def tokenize(value: str, separator: str) -> list[str]:
    parts = re.split(separator, value.lower())
    return [part for part in parts if part]


def singularize(token: str) -> str:
    if len(token) > 3 and token.endswith("s"):
        return token[:-1]
    return token


def expand_query_tokens(query: str, separator: str) -> list[str]:
    base_tokens = tokenize(query, separator)
    expanded: list[str] = []
    for token in base_tokens:
        expanded.append(token)
        singular = singularize(token)
        if singular != token:
            expanded.append(singular)
        if token == "vscode":
            expanded.extend(["vs", "code"])
    return list(dict.fromkeys(expanded))


def collapse(value: str) -> str:
    return re.sub(r"[\W_]+", "", value.lower())


def location_base(location: str) -> str:
    return location.split("#", 1)[0]


def score_doc(query_tokens: list[str], query: str, doc: dict[str, str]) -> int:
    title = doc.get("title", "").lower()
    location = doc.get("location", "").lower()
    text = doc.get("text", "").lower()
    collapsed_query = collapse(query)
    collapsed_title = collapse(title)
    collapsed_location = collapse(location)
    collapsed_text = collapse(text[:4000])
    score = 0
    for token in query_tokens:
        if token in title:
            score += 10
        if token in location:
            score += 6
        if token in text:
            score += 2
        if singularize(token) in title:
            score += 4
        if singularize(token) in location:
            score += 3
    if all(token in title for token in query_tokens):
        score += 20
    if all(token in location for token in query_tokens):
        score += 12
    if query.lower() in title:
        score += 20
    if query.lower() in location:
        score += 12
    if query.lower() in text:
        score += 5
    if collapsed_query and collapsed_query in collapsed_title:
        score += 25
    if collapsed_query and collapsed_query in collapsed_location:
        score += 20
    if collapsed_query and collapsed_query in collapsed_text:
        score += 8
    base_location = location_base(location)
    if len(query_tokens) == 1 and f"/{query_tokens[0]}/" in base_location:
        score += 18
    if base_location.startswith("reference/config/") and any(
        token in query_tokens for token in ("retain", "watchdog", "fault", "toml", "tls", "mesh")
    ):
        score += 12
    if base_location.startswith("reference/config/runtime-toml/") and any(
        token in query_tokens for token in ("retain", "watchdog", "fault")
    ):
        score += 30
    if base_location.startswith("reference/specifications/08-standard-function-blocks/") and any(
        token in query_tokens for token in ("ctu", "ctd", "ctud", "ton", "tof", "tp", "timer")
    ):
        score += 30
    if (
        any(re.fullmatch(r"[ewi]\d{3}", token) for token in query_tokens)
        and base_location.startswith("reference/diagnostics/")
    ):
        score += 40
    if base_location.startswith("faq/") and "faq" not in query.lower():
        score -= 30
    if base_location.startswith("examples/") and not any(
        keyword in query.lower() for keyword in ("example", "examples", "tutorial", "tutorials")
    ):
        score -= 20
    if base_location.startswith("changelog/") and "changelog" not in query.lower():
        score -= 25
    if base_location.startswith("maintaining/") and "maintaining" not in query.lower():
        score -= 25
    return score


def main() -> int:
    if not SEARCH_INDEX.exists():
        print(f"missing search index: {SEARCH_INDEX}", file=sys.stderr)
        return 1

    payload = json.loads(SEARCH_INDEX.read_text(encoding="utf-8"))
    docs: list[dict[str, str]] = payload["docs"]
    separator = payload.get("config", {}).get("separator", r"[\s\-\.]+")

    failures: list[str] = []
    for query, expected in QUERY_EXPECTATIONS:
        query_tokens = expand_query_tokens(query, separator)
        ranked_by_base: dict[str, tuple[int, dict[str, str]]] = {}
        for doc in docs:
            score = score_doc(query_tokens, query, doc)
            base = location_base(doc.get("location", ""))
            current = ranked_by_base.get(base)
            if current is None or score > current[0]:
                ranked_by_base[base] = (score, doc)

        ranked = sorted(
            ranked_by_base.values(),
            key=lambda item: item[0],
            reverse=True,
        )[:3]

        if not any(expected in doc.get("location", "") for _, doc in ranked):
            locations = ", ".join(doc.get("location", "<missing>") for _, doc in ranked)
            failures.append(
                f"query '{query}' did not rank expected page '{expected}' in top 3 (got: {locations})"
            )

    if failures:
        print("public docs search regression failed:", file=sys.stderr)
        for failure in failures:
            print(f"  - {failure}", file=sys.stderr)
        return 1

    print("public docs search regression passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
