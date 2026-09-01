import pathlib
import os
import subprocess
import tempfile
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[2]


class OpenOtDatabaseRunnerContractTests(unittest.TestCase):
    def test_workflow_owns_prepare_and_teardown_scripts(self) -> None:
        workflow = (ROOT / ".github/workflows/openot-real-databases.yml").read_text()

        self.assertIn("[self-hosted, Linux, X64, openot-databases]", workflow)
        self.assertIn("scripts/openot_database_runner_prepare.sh", workflow)
        self.assertIn("scripts/openot_database_runner_teardown.sh", workflow)
        self.assertNotIn("/opt/trust/openot-databases", workflow)

        prepare = ROOT / "scripts/openot_database_runner_prepare.sh"
        teardown = ROOT / "scripts/openot_database_runner_teardown.sh"
        self.assertTrue(prepare.is_file())
        self.assertTrue(teardown.is_file())

    def test_release_gate_checks_compiled_out_backend_startup(self) -> None:
        gate = (ROOT / "scripts/openot_real_database_gate.sh").read_text()

        self.assertIn("openot_compiled_out_backend", gate)
        self.assertIn("--no-default-features", gate)
        self.assertIn("--features openot-database-postgresql", gate)

    def test_prepare_contract_is_ephemeral_and_exports_every_backend(self) -> None:
        prepare = (ROOT / "scripts/openot_database_runner_prepare.sh").read_text()

        for product in (
            "POSTGRES",
            "TIMESCALE",
            "MYSQL",
            "MARIADB",
            "SQLSERVER",
            "INFLUX",
        ):
            self.assertIn(f"TRUST_TEST_OPENOT_{product}", prepare)
        self.assertIn("GITHUB_ENV", prepare)
        self.assertIn("openssl rand", prepare)
        self.assertIn("docker run", prepare)
        for digest in (
            "4ef4dbc939d61acea57712655ddb4b4ab27419c913f94cca0cd57cb3ea3c2280",
            "9508616d5b941ed931198504c5db3fb47e8f53f790732ea1e889591f1062057c",
            "b3b90af2a6552ae30c266fdb7d5dd55f3afb72404bb78d37fe8a23eb857fd3fb",
            "24e76fcec8c003a0362d0dd53f4806e7e79458d7fdeaf47437760e19496f5a9c",
            "2f9da673779dc5556d385164f6b1541d169ff1eeed97b9833ca0308e8628e683",
            "f4a6d4a76f0ed0a196cc997da472cd0b7ae52a766430493a1bead807ab8c1217",
            "b3c656d55d7ad751196f21b7fd2e8d4da9cb430e32f646adcf92441b72f82b14",
        ):
            self.assertIn(digest, prepare)
        self.assertIn("create token --admin", prepare)
        self.assertIn("--offline", prepare)
        self.assertIn("--admin-token-file", prepare)
        self.assertIn("unauthenticated InfluxDB write was accepted", prepare)
        self.assertIn("client_max_body_size 16m", prepare)
        self.assertIn("@127.0.0.1:53306/trust_logging", prepare)
        self.assertIn("@127.0.0.1:53307/trust_logging", prepare)
        self.assertNotIn('$state_dir/tls:/tls:ro', prepare)
        self.assertNotIn("OpenOtPassword", prepare)

    def test_teardown_contract_removes_all_owned_resources(self) -> None:
        teardown = (ROOT / "scripts/openot_database_runner_teardown.sh").read_text()

        self.assertIn("[[ -L $state_dir || -L $marker ]]", teardown)
        self.assertIn("docker rm -f", teardown)
        self.assertIn("docker volume rm", teardown)
        self.assertLess(
            teardown.index('docker inspect --format'),
            teardown.index('docker rm -f'),
        )
        self.assertIn("docker network rm", teardown)
        self.assertIn("OPENOT_DATABASE_RUNNER_STATE_DIR", teardown)
        self.assertIn(".trust-openot-runner-state", teardown)
        self.assertIn("validate_prefix", teardown)
        self.assertLess(
            teardown.index("[[ -L $state_dir || -L $marker ]]"),
            teardown.index('docker rm -f'),
        )
        self.assertLess(
            teardown.index('[[ $(<"$marker") != "$prefix" ]]'),
            teardown.index('docker rm -f'),
        )

    def test_teardown_rejects_symlinked_state_before_mutation(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            target = root / "target"
            target.mkdir()
            prefix = "trust-openot-symlink-test"
            (target / ".trust-openot-runner-state").write_text(f"{prefix}\n")
            state = root / "trust-openot-state"
            state.symlink_to(target, target_is_directory=True)
            environment = os.environ.copy()
            environment.update(
                OPENOT_DATABASE_RUNNER_STATE_DIR=str(state),
                OPENOT_DATABASE_RUNNER_PREFIX=prefix,
            )

            completed = subprocess.run(
                [str(ROOT / "scripts" / "openot_database_runner_teardown.sh")],
                cwd=ROOT,
                env=environment,
                check=False,
                capture_output=True,
                text=True,
            )

            self.assertEqual(completed.returncode, 2)
            self.assertIn("refusing symlinked OpenOT runner state", completed.stderr)
            self.assertTrue(state.is_symlink())
            self.assertTrue(target.is_dir())


if __name__ == "__main__":
    unittest.main()
