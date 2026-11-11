"""Every Ubuntu package install of the workflows and the gate scripts goes through scripts/ci_apt_install.sh.

On 11 November 2026 GitHub's hosted runners could not resolve the Ubuntu archives for hours, and every job that ran
`apt-get install` itself failed: Clippy, Conformance, Architecture Safety and the VS Code job (checklist G144). The
script makes the archives resolvable with public resolvers and retries the install.
"""

import re
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = "scripts/ci_apt_install.sh"
RAW_INSTALL = re.compile(r"\bapt(?:-get)?\s+(?:-\S+\s+)*install\b")


def sources() -> list[Path]:
    """The workflows and the shell scripts that CI runs."""
    workflows = sorted((ROOT / ".github" / "workflows").glob("*.y*ml"))
    scripts = sorted(path for path in (ROOT / "scripts").glob("*.sh") if path.name != Path(SCRIPT).name)
    return workflows + scripts


class CiAptInstallTests(unittest.TestCase):
    def test_no_workflow_or_script_installs_packages_with_apt_itself(self) -> None:
        """Catches an install that a runner's resolver outage fails, as every one did on 11 November."""
        raw = [
            f"{path.relative_to(ROOT)}:{number}: {line.strip()}"
            for path in sources()
            for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), start=1)
            if RAW_INSTALL.search(line) and not line.lstrip().startswith("#") and "Install with" not in line
        ]
        self.assertEqual(raw, [], "install Ubuntu packages with " + SCRIPT)

    def test_the_browser_install_with_deps_follows_the_script_in_its_job(self) -> None:
        """Playwright's `--with-deps` runs apt itself: the job's earlier script makes the archives resolvable for it.
        Catches the browser step moved before the script, or the script dropped from its job."""
        lines = (ROOT / ".github" / "workflows" / "ci.yml").read_text(encoding="utf-8").splitlines()
        uses = [index for index, line in enumerate(lines) if "--with-deps" in line and "capture:install" in line]
        self.assertTrue(uses, "ci.yml installs the browser with --with-deps")
        for index in uses:
            start = max(i for i in range(index + 1) if re.match(r"^  [A-Za-z0-9_-]+:\s*$", lines[i]))
            self.assertTrue(
                any(SCRIPT in line for line in lines[start:index]),
                f"ci.yml:{index + 1}: no {SCRIPT} before the browser's --with-deps in its job",
            )

    def test_the_script_makes_the_archives_resolvable_before_apt(self) -> None:
        """The order the outage needs: the resolver check and the public resolvers, then apt with retries."""
        text = (ROOT / SCRIPT).read_text(encoding="utf-8")
        check = text.index("if ! archives_resolve; then")
        install = text.index("apt-get -o Acquire::Retries=3 install")
        self.assertLess(check, install)
        for host in ("archive.ubuntu.com", "security.ubuntu.com", "azure.archive.ubuntu.com"):
            self.assertIn(host, text)


if __name__ == "__main__":
    unittest.main()
