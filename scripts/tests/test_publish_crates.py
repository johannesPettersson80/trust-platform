"""The publication of the SDK's crates to crates.io (specification 24, "Publishing the SDK's crates";
specification 48 RU-12): the order, the skip of a version crates.io holds, the run without a token and
crates.io's transient answers. cargo and crates.io are stood in for, so the tests publish nothing."""

from __future__ import annotations

import json
import re
import subprocess
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

import publish_crates  # noqa: E402

VERSION = "0.42.0"


class Cargo:
    """cargo as the script runs it: `metadata` answers the workspace's crates, `publish` is recorded."""

    def __init__(self, versions: dict[str, str] | None = None) -> None:
        self.versions = versions or {name: VERSION for name in publish_crates.CRATES}
        self.published: list[str] = []

    def __call__(self, args: list[str], **_: object) -> subprocess.CompletedProcess:
        if args[:2] == ["cargo", "metadata"]:
            packages = [{"name": name, "version": version} for name, version in self.versions.items()]
            packages.append({"name": "trust-runtime", "version": VERSION})
            return subprocess.CompletedProcess(args, 0, stdout=json.dumps({"packages": packages}))
        if args[:2] == ["cargo", "publish"]:
            self.published.append(args[args.index("--package") + 1])
            return subprocess.CompletedProcess(args, 0)
        raise AssertionError(f"unexpected command {args}")


class PublishCratesTests(unittest.TestCase):
    def test_the_crates_are_published_in_dependency_order_and_a_held_version_is_skipped(self) -> None:
        # trust-plc-lib needs trust-plc-abi, trust-plc-decl needs trust-plc-lib, trust-plc-macros needs
        # trust-plc-decl and trust-plc needs them all; trust-service needs trust-runtime-client: each is published
        # after those it depends on. A version
        # crates.io holds, here trust-plc-abi's from an earlier run, is skipped, so a re-run continues. Catches a
        # crate published before one it depends on, whose verification would not find it, and a re-run that
        # fails on the version an earlier run published.
        cargo = Cargo()
        asked: list[str] = []

        def fetch(url: str, user_agent: str) -> int:
            asked.append(url)
            self.assertIn("trust-platform-release-workflow", user_agent)
            return 200 if "/trust-plc-abi/" in url else 404

        code = publish_crates.main([], {"CARGO_REGISTRY_TOKEN": "token"}, cargo, fetch, lambda _: None)
        self.assertEqual(code, 0)
        self.assertEqual(
            cargo.published,
            [
                "trust-plc-lib",
                "trust-plc-decl",
                "trust-plc-macros",
                "trust-plc",
                "trust-runtime-client",
                "trust-service",
            ],
        )
        self.assertEqual(
            asked[0], f"https://crates.io/api/v1/crates/trust-plc-abi/{VERSION}"
        )
        self.assertEqual(len(asked), 7)

    def test_without_a_token_nothing_is_published_and_a_notice_says_why(self) -> None:
        # Only the owner of the crates' account gives the token: until the repository holds it, a release
        # publishes nothing to crates.io and says so. Catches a release that fails for the missing token, and
        # one that asks crates.io or cargo anything without it.
        cargo = Cargo()

        def fetch(url: str, user_agent: str) -> int:
            raise AssertionError("crates.io is not asked without a token")

        code = publish_crates.main([], {}, cargo, fetch, lambda _: None)
        self.assertEqual(code, 0)
        self.assertEqual(cargo.published, [])

    def test_a_transient_answer_is_asked_again_and_a_lasting_one_stops_the_publication(self) -> None:
        # A 503, then 404: the crate is published. Three answers that are neither 200 nor 404 stop the run before
        # anything more is published. Catches a run that takes an outage for an unpublished version, and one that
        # publishes past an answer it does not understand.
        cargo = Cargo()
        answers = iter([503, 404])
        pauses: list[float] = []
        published = publish_crates.is_published(
            "trust-plc-abi", VERSION, lambda url, agent: next(answers), "agent", pauses.append
        )
        self.assertFalse(published)
        self.assertEqual(pauses, [10])

        with self.assertRaises(SystemExit):
            publish_crates.main(
                [], {"CARGO_REGISTRY_TOKEN": "token"}, cargo, lambda url, agent: 500, lambda _: None
            )
        self.assertEqual(cargo.published, [])

    def test_crates_of_different_versions_are_refused(self) -> None:
        # The crates name each other at the release's exact version (RU-12): crates of different versions are no
        # release. Catches a publication of a version the release does not carry.
        versions = {name: VERSION for name in publish_crates.CRATES}
        versions["trust-plc-macros"] = "0.41.2"
        cargo = Cargo(versions)
        with self.assertRaises(SystemExit):
            publish_crates.main(
                [], {"CARGO_REGISTRY_TOKEN": "token"}, cargo, lambda url, agent: 404, lambda _: None
            )
        self.assertEqual(cargo.published, [])


if __name__ == "__main__":
    unittest.main()


class DocTestsInCiTests(unittest.TestCase):
    def test_ci_runs_the_doc_tests_of_every_crate_it_publishes(self) -> None:
        # Specification 48 RU-89: the SDK's crates are its API reference on docs.rs, and CI runs the documentation
        # examples of every crate RU-12 publishes. CI's Test job runs nextest, which runs no doc tests, so a published
        # crate that the doc-test step does not name has examples that no test runs. Catches a crate added to the
        # publication (trust-sim among them, when it joins) without its doc tests, and a step that no longer runs them.
        text = (ROOT / ".github" / "workflows" / "ci.yml").read_text(encoding="utf-8")
        steps = [line for line in text.replace("\\\n", " ").splitlines() if "cargo test --locked --doc" in line]
        self.assertEqual(len(steps), 1, "the one doc-test step of ci.yml")
        named = set(re.findall(r"-p ([a-z0-9-]+)", steps[0]))
        self.assertEqual([name for name in publish_crates.CRATES if name not in named], [])
