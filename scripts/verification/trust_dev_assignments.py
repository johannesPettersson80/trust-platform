"""Generate reviewed trust-dev assertion assignments in one batch."""

from __future__ import annotations

import argparse
import json
import tomllib
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

from scripts.verification.mapping_assertions import validate_assertion_batch
from scripts.verification.runtime_bytecode_assignments import generate_path_plans


AGENT = "crates/trust-dev/src/agent.rs"
CI = "crates/trust-dev/src/ci.rs"
COMMIT = "crates/trust-dev/src/commit.rs"
DOCS = "crates/trust-dev/src/docs/tests.rs"
TEST_CMD = "crates/trust-dev/src/test_cmd/tests.rs"

EXPECTED_PATH_COUNTS: Mapping[str, int] = {
    AGENT: 4,
    CI: 6,
    COMMIT: 6,
    DOCS: 4,
    TEST_CMD: 19,
}

BehaviorKey = tuple[str, str]
TestKey = tuple[str, str]

TEST_BINDINGS: Mapping[TestKey, BehaviorKey] = {
    (AGENT, "normalize_workspace_path_rejects_parent_escape"): (
        "DEV_AGENT_WORKSPACE_PATH_001",
        "PARENT_COMPONENT_ESCAPES_LEXICAL_ROOT",
    ),
    (AGENT, "normalize_workspace_path_collapses_current_dir_segments"): (
        "DEV_AGENT_WORKSPACE_PATH_001",
        "RELATIVE_PATH_WITH_CURRENT_OR_IN_SCOPE_PARENT_COMPONENT",
    ),
    (AGENT, "decode_json_value_supports_typed_scalars"): (
        "RT_HARNESS_BOUNDARY_IO_001",
        "TYPED_JSON_SCALAR_CODEC",
    ),
    (AGENT, "encode_value_emits_typed_payload"): (
        "RT_HARNESS_BOUNDARY_IO_001",
        "TYPED_JSON_SCALAR_CODEC",
    ),
    (CI, "classify_build_failure_code"): (
        "DEV_CI_EXIT_CLASSIFICATION_001",
        "ONE_RECOGNIZED_FAILURE_CLASS",
    ),
    (CI, "classify_test_failure_code"): (
        "DEV_CI_EXIT_CLASSIFICATION_001",
        "ONE_RECOGNIZED_FAILURE_CLASS",
    ),
    (CI, "classify_invalid_config_code"): (
        "DEV_CI_EXIT_CLASSIFICATION_001",
        "ONE_RECOGNIZED_FAILURE_CLASS",
    ),
    (CI, "classify_timeout_code"): (
        "DEV_CI_EXIT_CLASSIFICATION_001",
        "ONE_RECOGNIZED_FAILURE_CLASS",
    ),
    (CI, "classify_internal_code"): (
        "DEV_CI_EXIT_CLASSIFICATION_001",
        "ONE_RECOGNIZED_FAILURE_CLASS",
    ),
    (CI, "classify_with_command_falls_back_for_internal"): (
        "DEV_CI_EXIT_CLASSIFICATION_001",
        "UNCLASSIFIED_MESSAGE_WITH_OPTIONAL_COMMAND",
    ),
    (COMMIT, "commit_scopes_commit_to_project_path_without_sweeping_pre_staged_files"): (
        "DEV_COMMIT_SCOPE_001",
        "PRESTAGED_OUTSIDE_SCOPE",
    ),
    (COMMIT, "commit_rejects_pre_staged_path_inside_project_without_mutation"): (
        "DEV_COMMIT_SCOPE_001",
        "PRESTAGED_SCOPE_COLLISION",
    ),
    (COMMIT, "repository_root_commit_rejects_any_pre_staged_path"): (
        "DEV_COMMIT_SCOPE_001",
        "PRESTAGED_SCOPE_COLLISION",
    ),
    (COMMIT, "dry_run_with_pre_staged_collision_reports_without_mutation"): (
        "DEV_COMMIT_SCOPE_001",
        "PRESTAGED_SCOPE_COLLISION",
    ),
    (COMMIT, "git_status_decodes_quoted_porcelain_paths_for_summary"): (
        "DEV_COMMIT_SCOPE_001",
        "STATUS_PATH_FIDELITY",
    ),
    (COMMIT, "git_status_accepts_non_utf8_project_path_without_panic"): (
        "DEV_COMMIT_SCOPE_001",
        "STATUS_PATH_FIDELITY",
    ),
    (DOCS, "parser_extraction_for_tagged_comments"): (
        "DEV_DOCS_GENERATION_001",
        "TAG_EXTRACTION_AND_DIAGNOSTICS",
    ),
    (DOCS, "broken_tag_diagnostics_are_reported"): (
        "DEV_DOCS_GENERATION_001",
        "TAG_EXTRACTION_AND_DIAGNOSTICS",
    ),
    (DOCS, "markdown_output_snapshot"): (
        "DEV_DOCS_GENERATION_001",
        "MARKDOWN_RENDER_FIXTURE",
    ),
    (DOCS, "html_output_snapshot"): (
        "DEV_DOCS_GENERATION_001",
        "HTML_RENDER_FIXTURE",
    ),
    (TEST_CMD, "load_sources_finds_mixed_case_extensions_under_literal_glob_chars"): (
        "DEV_TEST_DISCOVERY_001",
        "SUPPORTED_EXTENSION_CASES",
    ),
    (TEST_CMD, "discovery_ignores_comments_after_test_name"): (
        "DEV_TEST_DISCOVERY_001",
        "TEST_POU_DECLARATION_EXTRACTION",
    ),
    (TEST_CMD, "discovery_finds_test_pous_with_namespace_qualification"): (
        "DEV_TEST_DISCOVERY_001",
        "TEST_POU_DECLARATION_EXTRACTION",
    ),
    (TEST_CMD, "execution_reports_assertion_failure_for_test_program"): (
        "DEV_TEST_EXECUTION_001",
        "PROGRAM_ASSERTION_RESULT",
    ),
    (TEST_CMD, "execution_runs_test_function_block"): (
        "DEV_TEST_EXECUTION_001",
        "FUNCTION_BLOCK_BODY_EXECUTION",
    ),
    (TEST_CMD, "execution_isolated_per_test_case"): (
        "DEV_TEST_EXECUTION_001",
        "COLD_CASE_ISOLATION",
    ),
    (TEST_CMD, "prepared_runtime_cold_restarts_between_cases"): (
        "DEV_TEST_EXECUTION_001",
        "COLD_CASE_ISOLATION",
    ),
    (
        TEST_CMD,
        "execute_test_case_keeps_unconfigured_test_program_out_of_default_runtime",
    ): (
        "DEV_TEST_EXECUTION_001",
        "CONFIGURATION_TEST_PROGRAM_BINDING",
    ),
    (
        TEST_CMD,
        "execute_test_case_runs_test_program_when_session_registers_extra_program_instance",
    ): (
        "DEV_TEST_EXECUTION_001",
        "CONFIGURATION_TEST_PROGRAM_BINDING",
    ),
    (TEST_CMD, "run_test_executes_test_program_when_configuration_is_present"): (
        "DEV_TEST_EXECUTION_001",
        "CONFIGURATION_TEST_PROGRAM_BINDING",
    ),
    (TEST_CMD, "execute_test_case_returns_execution_timeout_for_deadline_overrun"): (
        "DEV_TEST_EXECUTION_001",
        "CASE_DEADLINE",
    ),
    (TEST_CMD, "json_output_contract"): (
        "DEV_TEST_REPORTING_001",
        "MACHINE_REPORT_FORMATS",
    ),
    (TEST_CMD, "tap_output_contract"): (
        "DEV_TEST_REPORTING_001",
        "MACHINE_REPORT_FORMATS",
    ),
    (TEST_CMD, "junit_output_contract"): (
        "DEV_TEST_REPORTING_001",
        "MACHINE_REPORT_FORMATS",
    ),
    (TEST_CMD, "human_output_shows_failure_summary_with_source_context"): (
        "DEV_TEST_REPORTING_001",
        "HUMAN_AND_LIST_REPORTS",
    ),
    (TEST_CMD, "human_output_filter_zero_message_is_clear"): (
        "DEV_TEST_REPORTING_001",
        "HUMAN_AND_LIST_REPORTS",
    ),
    (TEST_CMD, "list_output_contract"): (
        "DEV_TEST_REPORTING_001",
        "HUMAN_AND_LIST_REPORTS",
    ),
    (TEST_CMD, "timeout_message_pluralization"): (
        "DEV_TEST_REPORTING_001",
        "HUMAN_AND_LIST_REPORTS",
    ),
    (TEST_CMD, "ci_mode_defaults_human_output_to_junit"): (
        "DEV_TEST_REPORTING_001",
        "CI_OUTPUT_SELECTION",
    ),
}

REVIEWED_DIRECT_ORACLE_SNAPSHOT = r"""
["crates/trust-dev/src/agent.rs","decode_json_value_supports_typed_scalars","assert_eq ! (decode_json_value (& json ! ({ \"type\" : \"BOOL\" , \"value\" : true })) . expect (\"bool\") , Value :: Bool (true))"]
["crates/trust-dev/src/agent.rs","decode_json_value_supports_typed_scalars","assert_eq ! (decode_json_value (& json ! ({ \"type\" : \"INT\" , \"value\" : 4 })) . expect (\"int\") , Value :: Int (4))"]
["crates/trust-dev/src/agent.rs","decode_json_value_supports_typed_scalars","assert_eq ! (decode_json_value (& json ! ({ \"type\" : \"STRING\" , \"value\" : \"hello\" })) . expect (\"string\") , Value :: String (\"hello\" . into ()))"]
["crates/trust-dev/src/agent.rs","encode_value_emits_typed_payload","assert_eq ! (encode_json_value (& Value :: Bool (true)) , json ! ({ \"type\" : \"BOOL\" , \"value\" : true }))"]
["crates/trust-dev/src/agent.rs","normalize_workspace_path_collapses_current_dir_segments","assert_eq ! (path , PathBuf :: from (\"src/main.st\"))"]
["crates/trust-dev/src/agent.rs","normalize_workspace_path_rejects_parent_escape","assert_eq ! (error . code , super :: ERROR_PATH_OUTSIDE_WORKSPACE)"]
["crates/trust-dev/src/ci.rs","classify_build_failure_code","assert_eq ! (classify_error (\"compile failed\") , EXIT_BUILD_FAILED)"]
["crates/trust-dev/src/ci.rs","classify_build_failure_code","assert_eq ! (classify_error (\"sources directory not found: /tmp/project/sources\") , EXIT_BUILD_FAILED)"]
["crates/trust-dev/src/ci.rs","classify_internal_code","assert_eq ! (classify_error (\"unexpected panic\") , EXIT_INTERNAL)"]
["crates/trust-dev/src/ci.rs","classify_invalid_config_code","assert_eq ! (classify_error (\"invalid project folder '/tmp/p': missing src/ directory\") , EXIT_INVALID_CONFIG)"]
["crates/trust-dev/src/ci.rs","classify_invalid_config_code","assert_eq ! (classify_error (\"missing runtime.toml at /tmp/project/runtime.toml\") , EXIT_INVALID_CONFIG)"]
["crates/trust-dev/src/ci.rs","classify_test_failure_code","assert_eq ! (classify_error (\"1 ST test(s) failed\") , EXIT_TEST_FAILED)"]
["crates/trust-dev/src/ci.rs","classify_test_failure_code","assert_eq ! (classify_error (\"RuntimeError::AssertionFailed(ASSERT_EQUAL...)\") , EXIT_TEST_FAILED)"]
["crates/trust-dev/src/ci.rs","classify_timeout_code","assert_eq ! (classify_error (\"operation timed out\") , EXIT_TIMEOUT)"]
["crates/trust-dev/src/ci.rs","classify_with_command_falls_back_for_internal","assert_eq ! (classify_error_with_command (\"bad bundle\" , Some (\"validate\")) , EXIT_INVALID_CONFIG)"]
["crates/trust-dev/src/ci.rs","classify_with_command_falls_back_for_internal","assert_eq ! (classify_error_with_command (\"expected expression\" , Some (\"build\")) , EXIT_BUILD_FAILED)"]
["crates/trust-dev/src/ci.rs","classify_with_command_falls_back_for_internal","assert_eq ! (classify_error_with_command (\"unexpected runtime issue\" , Some (\"test\")) , EXIT_TEST_FAILED)"]
["crates/trust-dev/src/commit.rs","commit_rejects_pre_staged_path_inside_project_without_mutation","assert ! (git_available () , \"git is required for commit-scope proof\")"]
["crates/trust-dev/src/commit.rs","commit_rejects_pre_staged_path_inside_project_without_mutation","assert ! (message . contains (\"project/Main.st\") && message . contains (\"pre-staged\") , \"collision diagnostic must name the path: {message}\")"]
["crates/trust-dev/src/commit.rs","commit_rejects_pre_staged_path_inside_project_without_mutation","assert ! (result . is_err () , \"in-scope staged collision must abort\")"]
["crates/trust-dev/src/commit.rs","commit_rejects_pre_staged_path_inside_project_without_mutation","assert ! (run_git (& repo , & [\"rev-list\" , \"--count\" , \"HEAD\"]) . trim () == \"1\" , \"collision must not create a commit\")"]
["crates/trust-dev/src/commit.rs","commit_rejects_pre_staged_path_inside_project_without_mutation","assert ! (staged . starts_with (\"A project/Main.st\") , \"index changed: {staged:?}\")"]
["crates/trust-dev/src/commit.rs","commit_scopes_commit_to_project_path_without_sweeping_pre_staged_files","assert ! (! committed . contains (\"unrelated.txt\") , \"pre-staged out-of-project file was swept into commit: {committed}\")"]
["crates/trust-dev/src/commit.rs","commit_scopes_commit_to_project_path_without_sweeping_pre_staged_files","assert ! (committed . contains (\"project/Main.st\") , \"project file should be committed: {committed}\")"]
["crates/trust-dev/src/commit.rs","commit_scopes_commit_to_project_path_without_sweeping_pre_staged_files","assert ! (git_available () , \"git is required for commit-scope proof\")"]
["crates/trust-dev/src/commit.rs","commit_scopes_commit_to_project_path_without_sweeping_pre_staged_files","assert ! (staged . starts_with (\"A unrelated.txt\") , \"unrelated file should remain staged after scoped commit: {staged:?}\")"]
["crates/trust-dev/src/commit.rs","dry_run_with_pre_staged_collision_reports_without_mutation","assert ! (git_available () , \"git is required for commit-scope proof\")"]
["crates/trust-dev/src/commit.rs","dry_run_with_pre_staged_collision_reports_without_mutation","assert ! (result . is_err () , \"dry-run must still report the collision\")"]
["crates/trust-dev/src/commit.rs","dry_run_with_pre_staged_collision_reports_without_mutation","assert_eq ! (after , before , \"dry-run collision mutated repository state\")"]
["crates/trust-dev/src/commit.rs","git_status_accepts_non_utf8_project_path_without_panic","assert ! (git_available () , \"git is required for commit-scope proof\")"]
["crates/trust-dev/src/commit.rs","git_status_accepts_non_utf8_project_path_without_panic","assert ! (result . is_ok () , \"git_status must not unwrap non-UTF-8 project paths\")"]
["crates/trust-dev/src/commit.rs","git_status_accepts_non_utf8_project_path_without_panic","assert_eq ! (entries . len () , 1)"]
["crates/trust-dev/src/commit.rs","git_status_decodes_quoted_porcelain_paths_for_summary","assert ! (entries . iter () . any (| entry | entry . path == \"project/Motor Ω.st\") , \"status entries should contain decoded display path, got {entries:?}\")"]
["crates/trust-dev/src/commit.rs","git_status_decodes_quoted_porcelain_paths_for_summary","assert ! (git_available () , \"git is required for commit-scope proof\")"]
["crates/trust-dev/src/commit.rs","repository_root_commit_rejects_any_pre_staged_path","assert ! (git_available () , \"git is required for commit-scope proof\")"]
["crates/trust-dev/src/commit.rs","repository_root_commit_rejects_any_pre_staged_path","assert ! (message . contains (\"Main.st\") && message . contains (\"pre-staged\") , \"root collision diagnostic must name the path: {message}\")"]
["crates/trust-dev/src/commit.rs","repository_root_commit_rejects_any_pre_staged_path","assert ! (result . is_err () , \"root staged collision must abort\")"]
["crates/trust-dev/src/commit.rs","repository_root_commit_rejects_any_pre_staged_path","assert ! (run_git (& repo , & [\"rev-list\" , \"--count\" , \"HEAD\"]) . trim () == \"1\" , \"collision must not create a commit\")"]
["crates/trust-dev/src/docs/tests.rs","broken_tag_diagnostics_are_reported","assert ! (joined . contains (\"@param `Z` does not match any declared parameter\"))"]
["crates/trust-dev/src/docs/tests.rs","broken_tag_diagnostics_are_reported","assert ! (joined . contains (\"@return used on non-returning PROGRAM `Main`\"))"]
["crates/trust-dev/src/docs/tests.rs","broken_tag_diagnostics_are_reported","assert ! (joined . contains (\"missing description for @brief\"))"]
["crates/trust-dev/src/docs/tests.rs","broken_tag_diagnostics_are_reported","assert ! (joined . contains (\"missing description for @param `MissingDescription`\"))"]
["crates/trust-dev/src/docs/tests.rs","broken_tag_diagnostics_are_reported","assert ! (joined . contains (\"unknown documentation tag `@mystery`\"))"]
["crates/trust-dev/src/docs/tests.rs","html_output_snapshot","assert_eq ! (actual , expected)"]
["crates/trust-dev/src/docs/tests.rs","markdown_output_snapshot","assert_eq ! (actual , expected)"]
["crates/trust-dev/src/docs/tests.rs","parser_extraction_for_tagged_comments","assert_eq ! (diagnostics . len () , 0)"]
["crates/trust-dev/src/docs/tests.rs","parser_extraction_for_tagged_comments","assert_eq ! (function . kind , ApiItemKind :: Function)"]
["crates/trust-dev/src/docs/tests.rs","parser_extraction_for_tagged_comments","assert_eq ! (function . qualified_name , \"Add\")"]
["crates/trust-dev/src/docs/tests.rs","parser_extraction_for_tagged_comments","assert_eq ! (function . tags . brief . as_deref () , Some (\"Adds two numbers.\"))"]
["crates/trust-dev/src/docs/tests.rs","parser_extraction_for_tagged_comments","assert_eq ! (function . tags . params . len () , 2)"]
["crates/trust-dev/src/docs/tests.rs","parser_extraction_for_tagged_comments","assert_eq ! (function . tags . params [0] . name , \"A\")"]
["crates/trust-dev/src/docs/tests.rs","parser_extraction_for_tagged_comments","assert_eq ! (function . tags . params [1] . name , \"B\")"]
["crates/trust-dev/src/docs/tests.rs","parser_extraction_for_tagged_comments","assert_eq ! (function . tags . returns . as_deref () , Some (\"Sum value.\"))"]
["crates/trust-dev/src/docs/tests.rs","parser_extraction_for_tagged_comments","assert_eq ! (items . len () , 1)"]
["crates/trust-dev/src/test_cmd/tests.rs","ci_mode_defaults_human_output_to_junit","assert_eq ! (effective_output (TestOutput :: Human , false) , TestOutput :: Human)"]
["crates/trust-dev/src/test_cmd/tests.rs","ci_mode_defaults_human_output_to_junit","assert_eq ! (effective_output (TestOutput :: Human , true) , TestOutput :: Junit)"]
["crates/trust-dev/src/test_cmd/tests.rs","ci_mode_defaults_human_output_to_junit","assert_eq ! (effective_output (TestOutput :: Json , true) , TestOutput :: Json)"]
["crates/trust-dev/src/test_cmd/tests.rs","ci_mode_defaults_human_output_to_junit","assert_eq ! (effective_output (TestOutput :: Junit , true) , TestOutput :: Junit)"]
["crates/trust-dev/src/test_cmd/tests.rs","ci_mode_defaults_human_output_to_junit","assert_eq ! (effective_output (TestOutput :: Tap , true) , TestOutput :: Tap)"]
["crates/trust-dev/src/test_cmd/tests.rs","discovery_finds_test_pous_with_namespace_qualification","assert_eq ! (discovered . len () , 2)"]
["crates/trust-dev/src/test_cmd/tests.rs","discovery_finds_test_pous_with_namespace_qualification","assert_eq ! (discovered [0] . kind , TestKind :: FunctionBlock)"]
["crates/trust-dev/src/test_cmd/tests.rs","discovery_finds_test_pous_with_namespace_qualification","assert_eq ! (discovered [0] . name , \"CaseOne\")"]
["crates/trust-dev/src/test_cmd/tests.rs","discovery_finds_test_pous_with_namespace_qualification","assert_eq ! (discovered [0] . source_line . as_deref () , Some (\"TEST_FUNCTION_BLOCK CaseOne\"))"]
["crates/trust-dev/src/test_cmd/tests.rs","discovery_finds_test_pous_with_namespace_qualification","assert_eq ! (discovered [1] . kind , TestKind :: Program)"]
["crates/trust-dev/src/test_cmd/tests.rs","discovery_finds_test_pous_with_namespace_qualification","assert_eq ! (discovered [1] . name , \"Plain\")"]
["crates/trust-dev/src/test_cmd/tests.rs","discovery_finds_test_pous_with_namespace_qualification","assert_eq ! (discovered [1] . source_line . as_deref () , Some (\"TEST_PROGRAM Plain\"))"]
["crates/trust-dev/src/test_cmd/tests.rs","discovery_ignores_comments_after_test_name","assert_eq ! (discovered . len () , 2)"]
["crates/trust-dev/src/test_cmd/tests.rs","discovery_ignores_comments_after_test_name","assert_eq ! (discovered [0] . name , \"InlineComment\")"]
["crates/trust-dev/src/test_cmd/tests.rs","discovery_ignores_comments_after_test_name","assert_eq ! (discovered [1] . name , \"NextLineComment\")"]
["crates/trust-dev/src/test_cmd/tests.rs","execute_test_case_keeps_unconfigured_test_program_out_of_default_runtime","assert ! (matches ! (& err , RuntimeError :: ControlError (message) if message . contains (\"unbound PROGRAM declaration(s) under CONFIGURATION\") && message . contains (\"Probe\")) , \"unexpected error: {err}\")"]
["crates/trust-dev/src/test_cmd/tests.rs","execute_test_case_keeps_unconfigured_test_program_out_of_default_runtime","assert_eq ! (tests . len () , 1)"]
["crates/trust-dev/src/test_cmd/tests.rs","execute_test_case_returns_execution_timeout_for_deadline_overrun","assert ! (matches ! (err , RuntimeError :: ExecutionTimeout))"]
["crates/trust-dev/src/test_cmd/tests.rs","execute_test_case_runs_test_program_when_session_registers_extra_program_instance","assert ! (matches ! (err , RuntimeError :: AssertionFailed (_)) , \"registered test program was not executed: {err}\")"]
["crates/trust-dev/src/test_cmd/tests.rs","execute_test_case_runs_test_program_when_session_registers_extra_program_instance","assert_eq ! (tests . len () , 1)"]
["crates/trust-dev/src/test_cmd/tests.rs","execution_isolated_per_test_case","assert_eq ! (runtime . storage () . get_global (\"Counter\") , Some (& Value :: Int (1)) , \"execution {execution} did not run from cold initial state\")"]
["crates/trust-dev/src/test_cmd/tests.rs","execution_isolated_per_test_case","assert_eq ! (tests . len () , 1)"]
["crates/trust-dev/src/test_cmd/tests.rs","execution_reports_assertion_failure_for_test_program","assert ! (matches ! (err , RuntimeError :: AssertionFailed (_)))"]
["crates/trust-dev/src/test_cmd/tests.rs","execution_reports_assertion_failure_for_test_program","assert_eq ! (tests . len () , 1)"]
["crates/trust-dev/src/test_cmd/tests.rs","execution_runs_test_function_block","assert ! (matches ! (err , RuntimeError :: AssertionFailed (_)) , \"function-block test body was not executed: {err}\")"]
["crates/trust-dev/src/test_cmd/tests.rs","execution_runs_test_function_block","assert_eq ! (tests . len () , 1)"]
["crates/trust-dev/src/test_cmd/tests.rs","human_output_filter_zero_message_is_clear","assert ! (plain . contains (\"(2 tests discovered, all filtered out)\"))"]
["crates/trust-dev/src/test_cmd/tests.rs","human_output_filter_zero_message_is_clear","assert ! (plain . contains (\"0 tests matched filter \\\"START\\\"\"))"]
["crates/trust-dev/src/test_cmd/tests.rs","human_output_shows_failure_summary_with_source_context","assert ! (plain . contains (\"1 passed, 1 failed, 1 errors (6ms)\"))"]
["crates/trust-dev/src/test_cmd/tests.rs","human_output_shows_failure_summary_with_source_context","assert ! (plain . contains (\"1. TEST_PROGRAM::FailCase @ tests.st:12\"))"]
["crates/trust-dev/src/test_cmd/tests.rs","human_output_shows_failure_summary_with_source_context","assert ! (plain . contains (\"2. TEST_FUNCTION_BLOCK::ErrCase @ fb_tests.st:20\"))"]
["crates/trust-dev/src/test_cmd/tests.rs","human_output_shows_failure_summary_with_source_context","assert ! (plain . contains (\"FAIL [2/3] TEST_PROGRAM::FailCase tests.st:12 [2ms]\"))"]
["crates/trust-dev/src/test_cmd/tests.rs","human_output_shows_failure_summary_with_source_context","assert ! (plain . contains (\"Failure summary:\"))"]
["crates/trust-dev/src/test_cmd/tests.rs","human_output_shows_failure_summary_with_source_context","assert ! (plain . contains (\"reason : ASSERT_EQUAL failed: expected <2> & got 3\"))"]
["crates/trust-dev/src/test_cmd/tests.rs","human_output_shows_failure_summary_with_source_context","assert ! (plain . contains (\"source : ASSERT_EQUAL(INT#2, X);\"))"]
["crates/trust-dev/src/test_cmd/tests.rs","json_output_contract","assert_eq ! (value [\"summary\"] [\"duration_ms\"] , 6)"]
["crates/trust-dev/src/test_cmd/tests.rs","json_output_contract","assert_eq ! (value [\"summary\"] [\"errors\"] , 1)"]
["crates/trust-dev/src/test_cmd/tests.rs","json_output_contract","assert_eq ! (value [\"summary\"] [\"failed\"] , 1)"]
["crates/trust-dev/src/test_cmd/tests.rs","json_output_contract","assert_eq ! (value [\"summary\"] [\"passed\"] , 1)"]
["crates/trust-dev/src/test_cmd/tests.rs","json_output_contract","assert_eq ! (value [\"summary\"] [\"total\"] , 3)"]
["crates/trust-dev/src/test_cmd/tests.rs","json_output_contract","assert_eq ! (value [\"tests\"] [0] [\"duration_ms\"] , 1)"]
["crates/trust-dev/src/test_cmd/tests.rs","json_output_contract","assert_eq ! (value [\"tests\"] [0] [\"status\"] , \"passed\")"]
["crates/trust-dev/src/test_cmd/tests.rs","json_output_contract","assert_eq ! (value [\"tests\"] [1] [\"duration_ms\"] , 2)"]
["crates/trust-dev/src/test_cmd/tests.rs","json_output_contract","assert_eq ! (value [\"tests\"] [1] [\"source\"] , \"ASSERT_EQUAL(INT#2, X);\")"]
["crates/trust-dev/src/test_cmd/tests.rs","json_output_contract","assert_eq ! (value [\"tests\"] [1] [\"status\"] , \"failed\")"]
["crates/trust-dev/src/test_cmd/tests.rs","json_output_contract","assert_eq ! (value [\"tests\"] [2] [\"duration_ms\"] , 3)"]
["crates/trust-dev/src/test_cmd/tests.rs","json_output_contract","assert_eq ! (value [\"tests\"] [2] [\"status\"] , \"error\")"]
["crates/trust-dev/src/test_cmd/tests.rs","json_output_contract","assert_eq ! (value [\"version\"] , 1)"]
["crates/trust-dev/src/test_cmd/tests.rs","junit_output_contract","assert ! (output . contains (\"<?xml version=\\\"1.0\\\" encoding=\\\"UTF-8\\\"?>\"))"]
["crates/trust-dev/src/test_cmd/tests.rs","junit_output_contract","assert ! (output . contains (\"<error message=\\\"runtime &lt;panic&gt;\\\">\"))"]
["crates/trust-dev/src/test_cmd/tests.rs","junit_output_contract","assert ! (output . contains (\"<failure message=\\\"ASSERT_EQUAL failed: expected &lt;2&gt; &amp; got 3\\\">\"))"]
["crates/trust-dev/src/test_cmd/tests.rs","junit_output_contract","assert ! (output . contains (\"<testcase name=\\\"TEST_PROGRAM::PassCase\\\"\"))"]
["crates/trust-dev/src/test_cmd/tests.rs","junit_output_contract","assert ! (output . contains (\"<testsuite name=\\\"trust-runtime\\\" tests=\\\"3\\\" failures=\\\"1\\\" errors=\\\"1\\\" skipped=\\\"0\\\">\"))"]
["crates/trust-dev/src/test_cmd/tests.rs","list_output_contract","assert ! (text . contains (\"2 test(s) listed\"))"]
["crates/trust-dev/src/test_cmd/tests.rs","list_output_contract","assert ! (text . contains (\"TEST_FUNCTION_BLOCK::CaseB (src/tests.st:24)\"))"]
["crates/trust-dev/src/test_cmd/tests.rs","list_output_contract","assert ! (text . contains (\"TEST_PROGRAM::CaseA (src/tests.st:1)\"))"]
["crates/trust-dev/src/test_cmd/tests.rs","load_sources_finds_mixed_case_extensions_under_literal_glob_chars","assert_eq ! (names , vec ! [\"Library.Pou\" , \"Motor.St\"])"]
["crates/trust-dev/src/test_cmd/tests.rs","prepared_runtime_cold_restarts_between_cases","assert_eq ! (runtime . storage () . get_global (\"Counter\") , Some (& Value :: Int (1)) , \"first test body did not execute\")"]
["crates/trust-dev/src/test_cmd/tests.rs","prepared_runtime_cold_restarts_between_cases","assert_eq ! (runtime . storage () . get_global (\"Counter\") , Some (& Value :: Int (2)) , \"second test did not execute after a cold restart\")"]
["crates/trust-dev/src/test_cmd/tests.rs","prepared_runtime_cold_restarts_between_cases","assert_eq ! (tests . len () , 2)"]
["crates/trust-dev/src/test_cmd/tests.rs","run_test_executes_test_program_when_configuration_is_present","assert ! (format ! (\"{error:#}\") . contains (\"1 ST test(s) failed\") , \"unexpected test command failure: {error:#}\")"]
["crates/trust-dev/src/test_cmd/tests.rs","tap_output_contract","assert ! (output . contains (\"# file: tests.st\"))"]
["crates/trust-dev/src/test_cmd/tests.rs","tap_output_contract","assert ! (output . contains (\"# line: 12\"))"]
["crates/trust-dev/src/test_cmd/tests.rs","tap_output_contract","assert ! (output . contains (\"# source: ASSERT_EQUAL(INT#2, X);\"))"]
["crates/trust-dev/src/test_cmd/tests.rs","tap_output_contract","assert ! (output . contains (\"not ok 2 - TEST_PROGRAM::FailCase\"))"]
["crates/trust-dev/src/test_cmd/tests.rs","tap_output_contract","assert ! (output . contains (\"not ok 3 - TEST_FUNCTION_BLOCK::ErrCase\"))"]
["crates/trust-dev/src/test_cmd/tests.rs","tap_output_contract","assert ! (output . contains (\"ok 1 - TEST_PROGRAM::PassCase\"))"]
["crates/trust-dev/src/test_cmd/tests.rs","tap_output_contract","assert ! (output . starts_with (\"TAP version 13\\n1..3\\n\"))"]
["crates/trust-dev/src/test_cmd/tests.rs","timeout_message_pluralization","assert_eq ! (timeout_message (1) , \"test timed out after 1 second\")"]
["crates/trust-dev/src/test_cmd/tests.rs","timeout_message_pluralization","assert_eq ! (timeout_message (5) , \"test timed out after 5 seconds\")"]
""".strip()


def combine_inventory_documents(
    documents: Sequence[Mapping[str, Any]],
) -> dict[str, Any]:
    inventories: dict[str, Any] = {}
    for document in documents:
        rows = document.get("inventories")
        if not isinstance(rows, Mapping):
            raise ValueError("trust-dev assertion document lacks inventories")
        for discovery_id, row in rows.items():
            if discovery_id in inventories:
                raise ValueError(f"duplicate trust-dev inventory {discovery_id}")
            inventories[str(discovery_id)] = row
    return {"schema_version": 2, "inventories": inventories}


def select_live_inventories(
    *,
    root: Path,
    inventory_document: Mapping[str, Any],
) -> dict[str, Any]:
    inventories = _canonical_saved_inventories(
        root=root,
        inventory_document=inventory_document,
    )
    denominator = tomllib.loads(
        (root / "verification/test-catalog-denominator.toml").read_text()
    )
    expected = {
        str(row["discovery_id"]): (str(row["path"]), str(row["name"]))
        for row in denominator.get("reviews", [])
        if isinstance(row, Mapping)
        and row.get("path") in EXPECTED_PATH_COUNTS
        and row.get("disposition") == "reviewed_nonmapping"
        and row.get("reason_code") == "no_reviewed_spec_or_invariant_binding"
    }
    if set(expected.values()) != set(TEST_BINDINGS):
        raise ValueError("live trust-dev denominator does not equal reviewed bindings")
    selected = {
        discovery_id: inventories[discovery_id]
        for discovery_id in sorted(expected)
        if discovery_id in inventories
    }
    if set(selected) != set(expected):
        raise ValueError("saved inventory lacks live trust-dev mapping debt")
    for discovery_id, raw in selected.items():
        if not isinstance(raw, Mapping):
            raise ValueError(f"{discovery_id} inventory is invalid")
        if (raw.get("path"), raw.get("test_name")) != expected[discovery_id]:
            raise ValueError(f"{discovery_id} inventory identity drifted")
    return {"schema_version": 2, "inventories": selected}


def generate_assignments(
    *,
    root: Path,
    inventory_document: Mapping[str, Any],
    reviewed_oracle_snapshot: str = REVIEWED_DIRECT_ORACLE_SNAPSHOT,
) -> dict[str, Any]:
    inventories = _canonical_saved_inventories(
        root=root,
        inventory_document=inventory_document,
    )
    behavior_indexes = _invariant_behavior_indexes(root)
    rows = []
    seen: set[TestKey] = set()
    direct_oracle_lines = []
    for discovery_id, raw in sorted(inventories.items()):
        path = _string(raw, "path", str(discovery_id))
        name = _string(raw, "test_name", str(discovery_id))
        key = (path, name)
        if key in seen or key not in TEST_BINDINGS:
            raise ValueError(f"unexpected or duplicate trust-dev test {path}:{name}")
        seen.add(key)
        if raw.get("unresolved_assertion_helpers") != []:
            raise ValueError(f"{path}:{name} has unresolved assertion helpers")
        assertions = raw.get("assertions")
        if (
            not isinstance(assertions, Sequence)
            or isinstance(assertions, (str, bytes))
            or not assertions
        ):
            raise ValueError(f"{path}:{name} lacks assertions")
        invariant_id, partition = TEST_BINDINGS[key]
        try:
            behavior_index = behavior_indexes[(invariant_id, partition)]
        except KeyError as error:
            raise ValueError(
                f"{path}:{name} selects missing behavior "
                f"{invariant_id}:{partition}"
            ) from error
        assertion_rows = []
        setup_ids = []
        direct_oracle_count = 0
        for assertion in assertions:
            if not isinstance(assertion, Mapping):
                raise ValueError(f"{path}:{name} assertion is invalid")
            assertion_id = _string(assertion, "assertion_id", name)
            expression = _string(assertion, "expression", assertion_id)
            function = _string(assertion, "function", assertion_id)
            is_direct_oracle = (
                function.rsplit("::", 1)[-1] == name
                and expression.startswith("assert")
            )
            if is_direct_oracle:
                direct_oracle_count += 1
                direct_oracle_lines.append(
                    json.dumps(
                        (path, name, expression),
                        ensure_ascii=False,
                        separators=(",", ":"),
                    )
                )
            else:
                setup_ids.append(assertion_id)
            assertion_rows.append(
                {
                    "assertion_id": assertion_id,
                    "invariant_id": invariant_id,
                    "behavior_index": behavior_index,
                }
            )
        if direct_oracle_count == 0:
            raise ValueError(f"{path}:{name} lacks a direct reviewed assertion oracle")
        rows.append(
            {
                "path": path,
                "name": name,
                "assertion_assignments": assertion_rows,
                "setup_assertion_ids": setup_ids,
                "non_oracle_assertion_ids": [],
            }
        )
    if seen != set(TEST_BINDINGS):
        raise ValueError("trust-dev assignments are not exhaustive")
    actual_oracle_snapshot = "\n".join(sorted(direct_oracle_lines))
    if actual_oracle_snapshot != reviewed_oracle_snapshot:
        raise ValueError("reviewed trust-dev direct oracle expressions drifted")
    return {
        "schema_version": 3,
        "basis": "reviewed_trust_dev_semantic_bindings",
        "assignments": rows,
    }


def _canonical_saved_inventories(
    *,
    root: Path,
    inventory_document: Mapping[str, Any],
) -> dict[str, dict[str, Any]]:
    if set(inventory_document) != {"schema_version", "inventories"}:
        raise ValueError("trust-dev assertion document fields drifted")
    if inventory_document.get("schema_version") != 2:
        raise ValueError("trust-dev assertion document schema drifted")
    inventories = inventory_document.get("inventories")
    if not isinstance(inventories, Mapping):
        raise ValueError("trust-dev assertion document lacks inventories")
    requested = []
    batch_rows = []
    saved: dict[str, dict[str, Any]] = {}
    for discovery_id, raw in sorted(inventories.items()):
        if not isinstance(discovery_id, str) or not discovery_id:
            raise ValueError("trust-dev inventory ID is invalid")
        if not isinstance(raw, Mapping):
            raise ValueError(f"{discovery_id} inventory is invalid")
        path = _string(raw, "path", discovery_id)
        name = _string(raw, "test_name", discovery_id)
        requested.append(
            {"discovery_id": discovery_id, "path": path, "name": name}
        )
        batch_rows.append(
            {key: value for key, value in raw.items() if key != "inventory_digest"}
        )
        saved[discovery_id] = dict(raw)
    canonical = validate_assertion_batch(
        root,
        {"schema_version": 2, "inventories": batch_rows},
        requested,
    )
    if canonical != saved:
        raise ValueError("saved trust-dev assertion inventory is not canonical")
    return canonical


def _invariant_behavior_indexes(root: Path) -> dict[BehaviorKey, int]:
    result: dict[BehaviorKey, int] = {}
    invariant_ids = {invariant_id for invariant_id, _ in TEST_BINDINGS.values()}
    for invariant_id in invariant_ids:
        matches = list(
            (root / "verification/invariants").glob(f"**/{invariant_id}.toml")
        )
        if len(matches) != 1:
            raise ValueError(f"expected one {invariant_id} invariant")
        record = tomllib.loads(matches[0].read_text())
        behaviors = record.get("behavior")
        if not isinstance(behaviors, list):
            raise ValueError(f"{invariant_id} lacks behaviors")
        for index, behavior in enumerate(behaviors):
            partition = behavior.get("partition") if isinstance(behavior, Mapping) else None
            name = partition.get("equals") if isinstance(partition, Mapping) else None
            if isinstance(name, str) and name:
                key = (invariant_id, name)
                if key in result:
                    raise ValueError(f"duplicate behavior partition {key}")
                result[key] = index
    return result


def _string(row: Mapping[str, Any], field: str, label: str) -> str:
    value = row.get(field)
    if not isinstance(value, str) or not value:
        raise ValueError(f"{label} lacks {field}")
    return value


def _rooted(root: Path, path: Path) -> Path:
    return path if path.is_absolute() else root / path


def _write_json(path: Path, document: Mapping[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(document, sort_keys=True, indent=2) + "\n")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path("."))
    parser.add_argument("--inventory", type=Path, action="append", required=True)
    parser.add_argument("--selected-inventory-out", type=Path, required=True)
    parser.add_argument("--assignments-out", type=Path, required=True)
    parser.add_argument("--execution-dir", type=Path)
    parser.add_argument("--plans-out", type=Path)
    parser.add_argument("--reviewed-date")
    parser.add_argument("--timestamp")
    args = parser.parse_args()
    root = args.root.resolve()
    combined = combine_inventory_documents(
        [json.loads(_rooted(root, path).read_text()) for path in args.inventory]
    )
    selected = select_live_inventories(
        root=root,
        inventory_document=combined,
    )
    assignments = generate_assignments(
        root=root,
        inventory_document=selected,
    )
    _write_json(_rooted(root, args.selected_inventory_out), selected)
    _write_json(_rooted(root, args.assignments_out), assignments)
    print(f"trust-dev assignments generated: {len(assignments['assignments'])}")
    plan_arguments = (
        args.execution_dir,
        args.plans_out,
        args.reviewed_date,
        args.timestamp,
    )
    if any(value is not None for value in plan_arguments):
        if not all(value is not None for value in plan_arguments):
            raise ValueError("trust-dev plan arguments must be complete")
        index = generate_path_plans(
            root=root,
            inventory_document=selected,
            assignments_document=assignments,
            execution_dir=_rooted(root, args.execution_dir),
            output_dir=_rooted(root, args.plans_out),
            reviewed_date=args.reviewed_date,
            timestamp=args.timestamp,
            target_profile="dev-remaining",
        )
        print(
            "trust-dev catalog plans generated: "
            f"{index['expected_count']} tests across {len(index['paths'])} paths"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
