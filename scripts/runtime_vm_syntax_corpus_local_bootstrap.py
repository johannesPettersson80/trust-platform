#!/usr/bin/env python3
from __future__ import annotations

import shutil
import sys
from pathlib import Path

ROOT_DIR = Path(__file__).resolve().parent.parent
DEFAULT_CORPUS_DIR = ROOT_DIR / "docs/internal/testing/local/runtime_vm_syntax_corpus"

COMMON_IO = """[io]
driver = "simulated"
params = {}

[[io.safe_state]]
address = "%QX0.0"
value = "FALSE"
"""

COMMON_RUNTIME_TEMPLATE = """[bundle]
version = 1

[resource]
name = "{resource_name}"
cycle_interval_ms = 10

[runtime]
execution_backend = "vm"

[runtime.control]
endpoint = "unix:///tmp/{endpoint}.sock"
mode = "production"
debug_enabled = false

[runtime.web]
enabled = false
listen = "127.0.0.1:8080"
auth = "local"
tls = false

[runtime.tls]
mode = "disabled"
require_remote = false

[runtime.discovery]
enabled = false
service_name = "truST"
advertise = false
interfaces = []

[runtime.mesh]
enabled = false
listen = "0.0.0.0:5200"
tls = false
auth_token = ""
publish = []

[runtime.opcua]
enabled = false
listen = "0.0.0.0:4840"
endpoint_path = "/"
namespace_uri = "urn:trust:runtime"
publish_interval_ms = 250
max_nodes = 128
expose = []
security_policy = "basic256sha256"
security_mode = "sign_and_encrypt"
allow_anonymous = false

[runtime.observability]
enabled = false
sample_interval_ms = 1000
mode = "all"
include = []
history_path = "history/historian.jsonl"
max_entries = 20000
prometheus_enabled = true
prometheus_path = "/metrics"

[runtime.log]
level = "info"

[runtime.retain]
mode = "none"
save_interval_ms = 1000

[runtime.watchdog]
enabled = false
timeout_ms = 1000
action = "halt"

[runtime.fault]
policy = "halt"
"""

COMMON_TRUST_LSP = """[project]
vendor_profile = "codesys"
include_paths = ["src"]
stdlib = "iec"
"""

COMMON_GLOBALS_PREFIX = """VAR_GLOBAL
    g_motion_bench_cycles : UDINT;
    g_motion_bench_completed_sequences : UDINT;
    g_motion_bench_last_error : WORD;
    g_motion_bench_power_on : BOOL;
    g_motion_bench_is_homed : BOOL;
    g_motion_bench_last_position : REAL;
    g_motion_bench_last_velocity : REAL;
    g_motion_bench_current_step : UINT;
"""
COMMON_GLOBALS_SUFFIX = "END_VAR\n"

PROJECTS: dict[str, dict[str, object]] = {
    "loop_arith": {
        "resource": "RuntimeVmSyntaxLoopArithRes",
        "endpoint": "trust-runtime-vm-syntax-loop-arith",
        "globals_extra": "    g_loop_arith_last_acc : DINT;\n    g_loop_arith_last_i : DINT;\n",
        "files": {
            "Main.st": """PROGRAM Main
VAR
    Initialized : BOOL;
    i : DINT;
    acc : DINT;
END_VAR

IF NOT Initialized THEN
    g_motion_bench_cycles := UDINT#0;
    g_motion_bench_completed_sequences := UDINT#0;
    g_motion_bench_last_error := WORD#16#0000;
    g_motion_bench_power_on := FALSE;
    g_motion_bench_is_homed := FALSE;
    g_motion_bench_last_position := REAL#0.0;
    g_motion_bench_last_velocity := REAL#0.0;
    g_motion_bench_current_step := UINT#3;
    g_loop_arith_last_acc := DINT#0;
    g_loop_arith_last_i := DINT#0;
    Initialized := TRUE;
END_IF;

g_motion_bench_cycles := g_motion_bench_cycles + UDINT#1;
i := DINT#0;
acc := DINT#0;
WHILE i < DINT#1000 DO
    acc := acc + i;
    i := i + DINT#1;
END_WHILE;

g_loop_arith_last_acc := acc;
g_loop_arith_last_i := i;
g_motion_bench_completed_sequences := DINT_TO_UDINT(acc);
g_motion_bench_last_error := WORD#16#0000;
g_motion_bench_power_on := FALSE;
g_motion_bench_is_homed := FALSE;
g_motion_bench_last_position := DINT_TO_REAL(acc);
g_motion_bench_last_velocity := DINT_TO_REAL(i);
g_motion_bench_current_step := UINT#3;
END_PROGRAM
"""
        },
    },
    "call_binding": {
        "resource": "RuntimeVmSyntaxCallBindingRes",
        "endpoint": "trust-runtime-vm-syntax-call-binding",
        "globals_extra": "    g_call_binding_last_v : INT;\n",
        "files": {
            "Main.st": """FUNCTION Add : INT
VAR_INPUT
    a : INT;
    b : INT := INT#2;
END_VAR

Add := a + b;
END_FUNCTION

FUNCTION Bump : INT
VAR_IN_OUT
    x : INT;
END_VAR
VAR_INPUT
    inc : INT := INT#1;
END_VAR

x := x + inc;
Bump := x;
END_FUNCTION

PROGRAM Main
VAR
    Initialized : BOOL;
    v : INT;
    out_named : INT;
    out_default : INT;
    out_positional : INT;
    out_inout : INT;
    total : UDINT;
END_VAR

IF NOT Initialized THEN
    g_motion_bench_cycles := UDINT#0;
    g_motion_bench_completed_sequences := UDINT#0;
    g_motion_bench_last_error := WORD#16#0000;
    g_motion_bench_power_on := FALSE;
    g_motion_bench_is_homed := FALSE;
    g_motion_bench_last_position := REAL#0.0;
    g_motion_bench_last_velocity := REAL#0.0;
    g_motion_bench_current_step := UINT#11;
    g_call_binding_last_v := INT#0;
    Initialized := TRUE;
END_IF;

g_motion_bench_cycles := g_motion_bench_cycles + UDINT#1;
v := INT#10;
out_named := Add(b := INT#4, a := INT#3);
out_default := Add(a := INT#3);
out_positional := Add(INT#5, INT#6);
out_inout := Bump(v, INT#5);
total := INT_TO_UDINT(out_named + out_default + out_positional + out_inout);
g_call_binding_last_v := v;
g_motion_bench_completed_sequences := total;
g_motion_bench_last_error := WORD#16#0000;
g_motion_bench_power_on := FALSE;
g_motion_bench_is_homed := FALSE;
g_motion_bench_last_position := INT_TO_REAL(out_positional);
g_motion_bench_last_velocity := INT_TO_REAL(out_inout);
g_motion_bench_current_step := UINT#11;
END_PROGRAM
"""
        },
    },
    "string_stdlib": {
        "resource": "RuntimeVmSyntaxStringStdlibRes",
        "endpoint": "trust-runtime-vm-syntax-string-stdlib",
        "globals_extra": "    g_string_stdlib_last_find : INT;\n",
        "files": {
            "Main.st": """PROGRAM Main
VAR
    Initialized : BOOL;
    out_left : STRING := '';
    out_mid : STRING := '';
    out_find_found : INT := INT#0;
    out_find_missing : INT := INT#0;
    out_w_replace : WSTRING := "";
    out_w_insert : WSTRING := "";
    total : UDINT;
END_VAR

IF NOT Initialized THEN
    g_motion_bench_cycles := UDINT#0;
    g_motion_bench_completed_sequences := UDINT#0;
    g_motion_bench_last_error := WORD#16#0000;
    g_motion_bench_power_on := FALSE;
    g_motion_bench_is_homed := FALSE;
    g_motion_bench_last_position := REAL#0.0;
    g_motion_bench_last_velocity := REAL#0.0;
    g_motion_bench_current_step := UINT#12;
    g_string_stdlib_last_find := INT#0;
    Initialized := TRUE;
END_IF;

g_motion_bench_cycles := g_motion_bench_cycles + UDINT#1;
out_left := LEFT(IN := 'ABCDE', L := INT#3);
out_mid := MID(IN := 'ABCDE', L := INT#2, P := INT#2);
out_find_found := FIND(IN1 := 'ABCDE', IN2 := 'BC');
out_find_missing := FIND(IN1 := 'BC', IN2 := 'ABCDE');
out_w_replace := REPLACE(IN1 := "ABCDE", IN2 := "Z", L := INT#2, P := INT#3);
out_w_insert := INSERT(IN1 := "ABE", IN2 := "CD", P := INT#3);
total := INT_TO_UDINT(LEN(out_left) + LEN(out_mid) + out_find_found + out_find_missing);
g_string_stdlib_last_find := out_find_found;
g_motion_bench_completed_sequences := total;
g_motion_bench_last_error := WORD#16#0000;
g_motion_bench_power_on := FALSE;
g_motion_bench_is_homed := FALSE;
g_motion_bench_last_position := INT_TO_REAL(out_find_found);
g_motion_bench_last_velocity := INT_TO_REAL(out_find_missing);
g_motion_bench_current_step := UINT#12;
END_PROGRAM
"""
        },
    },
    "refs_sizeof": {
        "resource": "RuntimeVmSyntaxRefsSizeofRes",
        "endpoint": "trust-runtime-vm-syntax-refs-sizeof",
        "globals_extra": "    g_refs_sizeof_last_value : INT;\n",
        "files": {
            "Types.st": """TYPE
    Inner : STRUCT
        arr : ARRAY[0..2] OF INT;
    END_STRUCT;
    Outer : STRUCT
        inner : Inner;
    END_STRUCT;
END_TYPE
""",
            "Main.st": """PROGRAM Main
VAR
    Initialized : BOOL;
    value_cell : INT := INT#4;
    other_cell : INT := INT#6;
    r_value : REF_TO INT;
    out_ref : INT := INT#0;
    out_after_write : INT := INT#0;
    out_second_ref : INT := INT#0;
    out_size_type_int : DINT := DINT#0;
    total : UDINT;
END_VAR

IF NOT Initialized THEN
    g_motion_bench_cycles := UDINT#0;
    g_motion_bench_completed_sequences := UDINT#0;
    g_motion_bench_last_error := WORD#16#0000;
    g_motion_bench_power_on := FALSE;
    g_motion_bench_is_homed := FALSE;
    g_motion_bench_last_position := REAL#0.0;
    g_motion_bench_last_velocity := REAL#0.0;
    g_motion_bench_current_step := UINT#13;
    g_refs_sizeof_last_value := INT#0;
    Initialized := TRUE;
END_IF;

g_motion_bench_cycles := g_motion_bench_cycles + UDINT#1;
value_cell := INT#4;
other_cell := INT#6;
r_value := REF(value_cell);
out_ref := r_value^;
r_value^ := r_value^ + INT#3;
out_after_write := r_value^;
r_value := REF(other_cell);
out_second_ref := r_value^;
out_size_type_int := SIZEOF(INT);
total := INT_TO_UDINT(out_ref + out_after_write + out_second_ref) + DINT_TO_UDINT(out_size_type_int);
g_refs_sizeof_last_value := out_after_write;
g_motion_bench_completed_sequences := total;
g_motion_bench_last_error := WORD#16#0000;
g_motion_bench_power_on := FALSE;
g_motion_bench_is_homed := FALSE;
g_motion_bench_last_position := INT_TO_REAL(out_second_ref);
g_motion_bench_last_velocity := DINT_TO_REAL(out_size_type_int);
g_motion_bench_current_step := UINT#13;
END_PROGRAM
"""
        },
    },
    "call_heavy_callee_arith": {
        "resource": "RuntimeVmSyntaxCallHeavyCalleeArithRes",
        "endpoint": "trust-runtime-vm-syntax-call-heavy-callee-arith",
        "globals_extra": "    g_call_heavy_last_value : INT;\n",
        "files": {
            "Main.st": """FUNCTION HotLoopFn : INT
VAR_INPUT
    a : INT;
    b : INT := INT#2;
END_VAR

HotLoopFn := a + b;
HotLoopFn := HotLoopFn + INT#1;
HotLoopFn := HotLoopFn + INT#2;
HotLoopFn := HotLoopFn + INT#3;
HotLoopFn := HotLoopFn + INT#4;
HotLoopFn := HotLoopFn + INT#5;
HotLoopFn := HotLoopFn + INT#6;
HotLoopFn := HotLoopFn + INT#7;
HotLoopFn := HotLoopFn + INT#8;
HotLoopFn := HotLoopFn + INT#9;
HotLoopFn := HotLoopFn + INT#10;
HotLoopFn := HotLoopFn + INT#11;
HotLoopFn := HotLoopFn + INT#12;
HotLoopFn := HotLoopFn + INT#13;
HotLoopFn := HotLoopFn + INT#14;
HotLoopFn := HotLoopFn + INT#15;
HotLoopFn := HotLoopFn + INT#16;
HotLoopFn := HotLoopFn + INT#17;
HotLoopFn := HotLoopFn + INT#18;
HotLoopFn := HotLoopFn + INT#19;
HotLoopFn := HotLoopFn + INT#20;
HotLoopFn := HotLoopFn - INT#1;
HotLoopFn := HotLoopFn - INT#2;
HotLoopFn := HotLoopFn - INT#3;
HotLoopFn := HotLoopFn - INT#4;
HotLoopFn := HotLoopFn - INT#5;
HotLoopFn := HotLoopFn - INT#6;
HotLoopFn := HotLoopFn - INT#7;
HotLoopFn := HotLoopFn - INT#8;
HotLoopFn := HotLoopFn - INT#9;
HotLoopFn := HotLoopFn - INT#10;
END_FUNCTION

PROGRAM Main
VAR
    Initialized : BOOL;
    out_named : INT := INT#0;
    out_default : INT := INT#0;
    out_positional : INT := INT#0;
    total : UDINT;
END_VAR

IF NOT Initialized THEN
    g_motion_bench_cycles := UDINT#0;
    g_motion_bench_completed_sequences := UDINT#0;
    g_motion_bench_last_error := WORD#16#0000;
    g_motion_bench_power_on := FALSE;
    g_motion_bench_is_homed := FALSE;
    g_motion_bench_last_position := REAL#0.0;
    g_motion_bench_last_velocity := REAL#0.0;
    g_motion_bench_current_step := UINT#14;
    g_call_heavy_last_value := INT#0;
    Initialized := TRUE;
END_IF;

g_motion_bench_cycles := g_motion_bench_cycles + UDINT#1;
out_named := HotLoopFn(b := INT#4, a := INT#3);
out_default := HotLoopFn(a := INT#3);
out_positional := HotLoopFn(INT#5, INT#6);
total := INT_TO_UDINT(out_named + out_default + out_positional);
g_call_heavy_last_value := out_positional;
g_motion_bench_completed_sequences := total;
g_motion_bench_last_error := WORD#16#0000;
g_motion_bench_power_on := FALSE;
g_motion_bench_is_homed := FALSE;
g_motion_bench_last_position := INT_TO_REAL(out_named);
g_motion_bench_last_velocity := INT_TO_REAL(out_positional);
g_motion_bench_current_step := UINT#14;
END_PROGRAM
"""
        },
    },
    "branch_control": {
        "resource": "RuntimeVmSyntaxBranchControlRes",
        "endpoint": "trust-runtime-vm-syntax-branch-control",
        "globals_extra": "    g_branch_control_last_acc : DINT;\n",
        "files": {
            "Main.st": """PROGRAM Main
VAR
    Initialized : BOOL;
    i : DINT;
    selector : DINT;
    acc : DINT;
    branch_hits : DINT;
END_VAR

IF NOT Initialized THEN
    g_motion_bench_cycles := UDINT#0;
    g_motion_bench_completed_sequences := UDINT#0;
    g_motion_bench_last_error := WORD#16#0000;
    g_motion_bench_power_on := FALSE;
    g_motion_bench_is_homed := FALSE;
    g_motion_bench_last_position := REAL#0.0;
    g_motion_bench_last_velocity := REAL#0.0;
    g_motion_bench_current_step := UINT#15;
    g_branch_control_last_acc := DINT#0;
    Initialized := TRUE;
END_IF;

g_motion_bench_cycles := g_motion_bench_cycles + UDINT#1;
i := DINT#0;
acc := DINT#0;
branch_hits := DINT#0;
WHILE i < DINT#256 DO
    selector := i MOD DINT#4;
    CASE selector OF
        DINT#0: acc := acc + i;
        DINT#1: acc := acc - i;
        DINT#2: acc := acc + DINT#3;
    ELSE
        acc := acc + DINT#7;
    END_CASE;

    IF acc < DINT#0 THEN
        acc := acc + DINT#11;
    ELSIF acc > DINT#10000 THEN
        acc := acc - DINT#13;
    ELSE
        branch_hits := branch_hits + DINT#1;
    END_IF;

    i := i + DINT#1;
END_WHILE;

g_branch_control_last_acc := acc;
g_motion_bench_completed_sequences := DINT_TO_UDINT(branch_hits);
g_motion_bench_last_error := WORD#16#0000;
g_motion_bench_power_on := FALSE;
g_motion_bench_is_homed := FALSE;
g_motion_bench_last_position := DINT_TO_REAL(acc);
g_motion_bench_last_velocity := DINT_TO_REAL(branch_hits);
g_motion_bench_current_step := UINT#15;
END_PROGRAM
"""
        },
    },
    "composite_updates": {
        "resource": "RuntimeVmSyntaxCompositeUpdatesRes",
        "endpoint": "trust-runtime-vm-syntax-composite-updates",
        "globals_extra": "    g_composite_updates_last_total : DINT;\n",
        "files": {
            "Types.st": """TYPE
    BenchCell : STRUCT
        Value : DINT;
        Flags : WORD;
    END_STRUCT;
    BenchFrame : STRUCT
        Cells : ARRAY[0..7] OF BenchCell;
        Total : DINT;
    END_STRUCT;
END_TYPE
""",
            "Main.st": """PROGRAM Main
VAR
    Initialized : BOOL;
    Values : ARRAY[0..7] OF DINT;
    Flags : ARRAY[0..7] OF WORD;
    idx : INT;
    acc : DINT;
END_VAR

IF NOT Initialized THEN
    g_motion_bench_cycles := UDINT#0;
    g_motion_bench_completed_sequences := UDINT#0;
    g_motion_bench_last_error := WORD#16#0000;
    g_motion_bench_power_on := FALSE;
    g_motion_bench_is_homed := FALSE;
    g_motion_bench_last_position := REAL#0.0;
    g_motion_bench_last_velocity := REAL#0.0;
    g_motion_bench_current_step := UINT#16;
    g_composite_updates_last_total := DINT#0;
    Initialized := TRUE;
END_IF;

g_motion_bench_cycles := g_motion_bench_cycles + UDINT#1;
idx := INT#0;
acc := DINT#0;
WHILE idx < INT#8 DO
    Values[idx] := INT_TO_DINT(idx) * DINT#3;
    Flags[idx] := INT_TO_WORD(idx) OR WORD#16#0001;
    acc := acc + Values[idx] + WORD_TO_DINT(Flags[idx]);
    idx := idx + INT#1;
END_WHILE;
g_composite_updates_last_total := acc;
g_motion_bench_completed_sequences := DINT_TO_UDINT(acc);
g_motion_bench_last_error := WORD#16#0000;
g_motion_bench_power_on := FALSE;
g_motion_bench_is_homed := FALSE;
g_motion_bench_last_position := DINT_TO_REAL(Values[6]);
g_motion_bench_last_velocity := DINT_TO_REAL(acc);
g_motion_bench_current_step := UINT#16;
END_PROGRAM
"""
        },
    },
    "time_date_stdlib": {
        "resource": "RuntimeVmSyntaxTimeDateStdlibRes",
        "endpoint": "trust-runtime-vm-syntax-time-date-stdlib",
        "globals_extra": "    g_time_date_stdlib_last_total : DINT;\n",
        "files": {
            "Main.st": """PROGRAM Main
VAR
    Initialized : BOOL;
    date_a : DATE := DATE#1970-01-02;
    tod_a : TOD := TOD#01:02:03.004;
    dt_a : DT := DT#1970-01-02-01:02:03.004;
    year_a : INT;
    month_a : INT;
    day_a : INT;
    hour_a : INT;
    minute_a : INT;
    second_a : INT;
    msec_a : INT;
    year_b : INT;
    month_b : INT;
    day_b : INT;
    hour_b : INT;
    minute_b : INT;
    second_b : INT;
    msec_b : INT;
    dow : INT;
    total : DINT;
END_VAR

IF NOT Initialized THEN
    g_motion_bench_cycles := UDINT#0;
    g_motion_bench_completed_sequences := UDINT#0;
    g_motion_bench_last_error := WORD#16#0000;
    g_motion_bench_power_on := FALSE;
    g_motion_bench_is_homed := FALSE;
    g_motion_bench_last_position := REAL#0.0;
    g_motion_bench_last_velocity := REAL#0.0;
    g_motion_bench_current_step := UINT#18;
    g_time_date_stdlib_last_total := DINT#0;
    Initialized := TRUE;
END_IF;

g_motion_bench_cycles := g_motion_bench_cycles + UDINT#1;
SPLIT_DATE(date_a, year_a, month_a, day_a);
SPLIT_TOD(tod_a, hour_a, minute_a, second_a, msec_a);
SPLIT_DT(dt_a, year_b, month_b, day_b, hour_b, minute_b, second_b, msec_b);
dow := DAY_OF_WEEK(date_a);
total := INT_TO_DINT(year_a + month_a + day_a + hour_a + minute_a + second_a + msec_a + year_b + month_b + day_b + hour_b + minute_b + second_b + msec_b + dow);
g_time_date_stdlib_last_total := total;
g_motion_bench_completed_sequences := DINT_TO_UDINT(total);
g_motion_bench_last_error := WORD#16#0000;
g_motion_bench_power_on := FALSE;
g_motion_bench_is_homed := FALSE;
g_motion_bench_last_position := DINT_TO_REAL(total);
g_motion_bench_last_velocity := INT_TO_REAL(hour_b + minute_b + second_b + msec_b);
g_motion_bench_current_step := UINT#18;
END_PROGRAM
"""
        },
    },
    "method_receiver": {
        "resource": "RuntimeVmSyntaxMethodReceiverRes",
        "endpoint": "trust-runtime-vm-syntax-method-receiver",
        "globals_extra": "    g_method_receiver_last_total : INT;\n",
        "files": {
            "Main.st": """INTERFACE ICounter
METHOD Inc : INT
VAR_INPUT
    delta : INT;
END_VAR
END_METHOD
END_INTERFACE

CLASS Counter IMPLEMENTS ICounter
VAR PUBLIC
    value : INT := INT#0;
END_VAR
METHOD PUBLIC Inc : INT
VAR_INPUT
    delta : INT;
END_VAR
value := value + delta;
Inc := value;
END_METHOD
END_CLASS

FUNCTION_BLOCK ThisCounter
VAR
    count : INT := INT#5;
END_VAR
METHOD PUBLIC Current : INT
Current := THIS.count;
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK BaseFb
VAR PUBLIC
    count : INT := INT#10;
END_VAR
METHOD PUBLIC GetCount : INT
GetCount := count;
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK DerivedFb EXTENDS BaseFb
VAR PUBLIC
    extra : INT := INT#3;
END_VAR
METHOD PUBLIC GetCount : INT
GetCount := count + extra;
END_METHOD
METHOD PUBLIC GetSuper : INT
GetSuper := SUPER.GetCount();
END_METHOD
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    Initialized : BOOL;
    i : ICounter;
    c : Counter;
    fb_this : ThisCounter;
    fb_derived : DerivedFb;
    out_this : INT := INT#0;
    out_override : INT := INT#0;
    out_super : INT := INT#0;
    out_iface : INT := INT#0;
    out_direct : INT := INT#0;
    total : INT := INT#0;
END_VAR

IF NOT Initialized THEN
    g_motion_bench_cycles := UDINT#0;
    g_motion_bench_completed_sequences := UDINT#0;
    g_motion_bench_last_error := WORD#16#0000;
    g_motion_bench_power_on := FALSE;
    g_motion_bench_is_homed := FALSE;
    g_motion_bench_last_position := REAL#0.0;
    g_motion_bench_last_velocity := REAL#0.0;
    g_motion_bench_current_step := UINT#19;
    g_method_receiver_last_total := INT#0;
    Initialized := TRUE;
END_IF;

g_motion_bench_cycles := g_motion_bench_cycles + UDINT#1;
i := c;
out_this := fb_this.Current();
out_override := fb_derived.GetCount();
out_super := fb_derived.GetSuper();
out_iface := i.Inc(INT#1);
out_direct := c.Inc(INT#2);
total := out_this + out_override + out_super + out_iface + out_direct;
g_method_receiver_last_total := total;
g_motion_bench_completed_sequences := INT_TO_UDINT(total);
g_motion_bench_last_error := WORD#16#0000;
g_motion_bench_power_on := FALSE;
g_motion_bench_is_homed := FALSE;
g_motion_bench_last_position := INT_TO_REAL(out_override);
g_motion_bench_last_velocity := INT_TO_REAL(out_direct);
g_motion_bench_current_step := UINT#19;
END_PROGRAM
"""
        },
    },
    "bitwise_conversions": {
        "resource": "RuntimeVmSyntaxBitwiseConversionsRes",
        "endpoint": "trust-runtime-vm-syntax-bitwise-conversions",
        "globals_extra": "    g_bitwise_conversions_last_value : DINT;\n",
        "files": {
            "Main.st": """PROGRAM Main
VAR
    Initialized : BOOL;
    mask_a : WORD;
    mask_b : WORD;
    out_and : WORD;
    out_or : WORD;
    out_xor : WORD;
    out_not : WORD;
    out_uint : UINT;
    out_dint : DINT;
END_VAR

IF NOT Initialized THEN
    g_motion_bench_cycles := UDINT#0;
    g_motion_bench_completed_sequences := UDINT#0;
    g_motion_bench_last_error := WORD#16#0000;
    g_motion_bench_power_on := FALSE;
    g_motion_bench_is_homed := FALSE;
    g_motion_bench_last_position := REAL#0.0;
    g_motion_bench_last_velocity := REAL#0.0;
    g_motion_bench_current_step := UINT#17;
    g_bitwise_conversions_last_value := DINT#0;
    Initialized := TRUE;
END_IF;

g_motion_bench_cycles := g_motion_bench_cycles + UDINT#1;
mask_a := WORD#16#00F3;
mask_b := WORD#16#0F30;
out_and := mask_a AND mask_b;
out_or := mask_a OR mask_b;
out_xor := mask_a XOR mask_b;
out_not := NOT mask_a;
out_uint := WORD_TO_UINT(out_and) + WORD_TO_UINT(out_xor);
out_dint := UINT_TO_DINT(out_uint) + WORD_TO_DINT(out_or) + WORD_TO_DINT(out_not AND WORD#16#00FF);
g_bitwise_conversions_last_value := out_dint;
g_motion_bench_completed_sequences := DINT_TO_UDINT(out_dint);
g_motion_bench_last_error := WORD#16#0000;
g_motion_bench_power_on := FALSE;
g_motion_bench_is_homed := FALSE;
g_motion_bench_last_position := DINT_TO_REAL(out_dint);
g_motion_bench_last_velocity := UINT_TO_REAL(out_uint);
g_motion_bench_current_step := UINT#17;
END_PROGRAM
"""
        },
    },
}


def write_project(root: Path, name: str, spec: dict[str, object]) -> None:
    project = root / name
    project.mkdir(parents=True, exist_ok=True)
    (project / "src").mkdir(exist_ok=True)
    (project / "io.toml").write_text(COMMON_IO)
    (project / "runtime.toml").write_text(
        COMMON_RUNTIME_TEMPLATE.format(
            resource_name=spec["resource"], endpoint=spec["endpoint"]
        )
    )
    (project / "trust-lsp.toml").write_text(COMMON_TRUST_LSP)
    (project / "src" / "Globals.st").write_text(
        COMMON_GLOBALS_PREFIX + str(spec["globals_extra"]) + COMMON_GLOBALS_SUFFIX
    )
    for filename, content in dict(spec["files"]).items():
        (project / "src" / filename).write_text(content)


def main() -> None:
    corpus_root = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else DEFAULT_CORPUS_DIR
    if corpus_root.exists():
        shutil.rmtree(corpus_root)
    corpus_root.mkdir(parents=True, exist_ok=True)
    for name, spec in PROJECTS.items():
        write_project(corpus_root, name, spec)
    (corpus_root / "README.local.md").write_text(
        "Local generated runtime VM syntax corpus. Recreate with scripts/bootstrap_runtime_vm_syntax_corpus_local.sh\n"
    )
    print(corpus_root)


if __name__ == "__main__":
    main()
