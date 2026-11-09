//! Specification 06 §15: CODESYS's `__TRY`, `__CATCH`, `__FINALLY` and `__ENDTRY`, run compiled. Every
//! expected value is worked out by hand from the programs below.
use trust_runtime::harness::{CompileSession, SourceFile, TestHarness};
use trust_runtime::value::{Duration, Value};
use trust_runtime::CompiledRuntime;

/// The configuration that runs program `Main` as `P1`.
const CONFIGURATION: &str = r"
CONFIGURATION Plant
RESOURCE Res ON PLC
    TASK Cycle (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH Cycle : Main;
END_RESOURCE
END_CONFIGURATION
";

fn runtime(program: &str) -> CompiledRuntime {
    TestHarness::from_session(CompileSession::from_sources(vec![SourceFile::with_path(
        "src/Main.st",
        format!("{program}\n{CONFIGURATION}"),
    )]))
    .unwrap_or_else(|error| panic!("the program compiles: {error}"))
    .into_runtime()
}

/// One scan, 100 ms of logical time after the last.
fn cycle(runtime: &mut CompiledRuntime) -> Result<(), String> {
    let now = runtime.current_time();
    runtime.set_current_time(Duration::from_nanos(
        now.as_nanos()
            .saturating_add(Duration::from_millis(100).as_nanos()),
    ));
    runtime.execute_cycle().map_err(|error| error.to_string())
}

/// The integer in `P1.<name>`.
fn int(runtime: &CompiledRuntime, name: &str) -> i64 {
    match runtime.read_variable(&format!("P1.{name}")).unwrap() {
        Value::Int(value) => i64::from(value),
        Value::UDInt(value) => i64::from(value),
        Value::DInt(value) => i64::from(value),
        other => panic!("P1.{name} holds {other:?}"),
    }
}

/// The handled errors and the flows of specification 06 §15, in one scan of one program.
const HANDLED: &str = r"
FUNCTION Early : INT
VAR_IN_OUT
    Leaves : INT;
END_VAR
__TRY
    Early := 1;
    RETURN;
__FINALLY
    Leaves := Leaves + 1;
__ENDTRY;
Early := 2;
END_FUNCTION

PROGRAM Main
VAR
    Zero : INT := 0;
    Result : INT := 7;
    Code : __SYSTEM.ExceptionCode;
    Caught : INT := 0;
    Finally : INT := 0;
    After : INT := 0;
    Clean : INT := 0;
    Loops : INT := 0;
    i : INT;
    Leaves : INT := 0;
    Returned : INT := 0;
    IsDivision : BOOL := FALSE;
END_VAR
(* A division by zero: the rest of the try statements is skipped, the catch runs, then the finally. *)
__TRY
    Result := 10 / Zero;
    Result := 99;
__CATCH(Code)
    Caught := Caught + 1;
    IsDivision := Code = __SYSTEM.ExceptionCode.RTSEXCPT_DIVIDEBYZERO;
__FINALLY
    Finally := Finally + 1;
__ENDTRY;
After := 1;
(* No error: the finally runs after the try statements. *)
__TRY
    Clean := 5;
__FINALLY
    Finally := Finally + 10;
__ENDTRY;
(* EXIT of the loop around the construct leaves after the finally. *)
FOR i := 1 TO 10 DO
    __TRY
        Loops := Loops + 1;
        IF i = 3 THEN
            EXIT;
        END_IF;
    __FINALLY
        Finally := Finally + 100;
    __ENDTRY;
END_FOR;
(* RETURN inside a function's try leaves after its finally. *)
Returned := Early(Leaves := Leaves);
END_PROGRAM
";

/// Worked from `HANDLED`, after one scan:
/// - `10 / Zero` raises a division by zero, so `Result` keeps 7 and `Result := 99` is skipped; the catch
///   runs once, `Code` is `RTSEXCPT_DIVIDEBYZERO` (16#102 = 258) and `IsDivision` TRUE; the scan goes on
///   (`After` is 1);
/// - `Finally` is 1 (the caught error) + 10 (no error) + 3 × 100 (the loop's three passes, the third
///   leaving by EXIT) = 311, and `Loops` is 3;
/// - `Early` returns 1 from inside its try, after its finally has counted `Leaves` to 1.
///
/// Wrong behaviour caught: a fault that ends the scan inside `__TRY`, a skipped `__FINALLY`, a catch
/// variable without CODESYS's code, an EXIT or a RETURN that skips the finally or does not leave.
#[test]
fn a_handled_error_runs_the_catch_and_the_finally_and_the_scan_goes_on() {
    let mut runtime = runtime(HANDLED);
    cycle(&mut runtime).expect("the scan handles its errors");
    assert_eq!(int(&runtime, "Result"), 7);
    assert_eq!(int(&runtime, "Caught"), 1);
    assert_eq!(int(&runtime, "Code"), 0x102);
    assert_eq!(
        runtime.read_variable("P1.IsDivision").unwrap(),
        Value::Bool(true)
    );
    assert_eq!(int(&runtime, "After"), 1);
    assert_eq!(int(&runtime, "Clean"), 5);
    assert_eq!(int(&runtime, "Finally"), 311);
    assert_eq!(int(&runtime, "Loops"), 3);
    assert_eq!(int(&runtime, "Returned"), 1);
    assert_eq!(int(&runtime, "Leaves"), 1);
}

/// Without `__CATCH`, the error runs the finally (`Finally` 1) and then faults the scan as it would
/// without `__TRY`; the statement after the construct never runs (`After` 0). Wrong behaviour caught: a
/// `__TRY` without a catch that swallows the error, or a fault that skips the finally.
#[test]
fn an_error_without_a_catch_runs_the_finally_and_faults_the_scan() {
    let mut runtime = runtime(
        r"
PROGRAM Main
VAR
    Zero : INT := 0;
    Result : INT := 7;
    Finally : INT := 0;
    After : INT := 0;
END_VAR
__TRY
    Result := 10 / Zero;
__FINALLY
    Finally := 1;
__ENDTRY;
After := 1;
END_PROGRAM
",
    );
    let error = cycle(&mut runtime).expect_err("the division by zero faults the scan");
    assert!(error.contains("division by zero"), "{error}");
    assert_eq!(int(&runtime, "Finally"), 1);
    assert_eq!(int(&runtime, "After"), 0);
}

/// The statement budget is never handled: a loop without end inside `__TRY` uses it up and ends the scan,
/// and the catch never runs (`Caught` 0). Wrong behaviour caught: a `__CATCH` that hides a program that
/// loops forever from the resource's protection.
#[test]
fn the_statement_budget_is_never_handled() {
    let mut runtime = runtime(
        r"
PROGRAM Main
VAR
    Code : __SYSTEM.ExceptionCode;
    Caught : INT := 0;
    Spin : DINT := 0;
END_VAR
__TRY
    WHILE TRUE DO
        Spin := Spin + 1;
    END_WHILE;
__CATCH(Code)
    Caught := 1;
__ENDTRY;
END_PROGRAM
",
    );
    let error = cycle(&mut runtime).expect_err("the budget ends the scan");
    assert!(
        error.contains("budget") || error.contains("infinite") || error.contains("limit"),
        "{error}"
    );
    assert_eq!(int(&runtime, "Caught"), 0);
}
