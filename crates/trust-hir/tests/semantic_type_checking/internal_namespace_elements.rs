//! INTERNAL elements of a namespace (IEC 61131-3 Ed.3 §6.9.2; specification 04, "Namespaces"): a
//! namespace's type, function, function block, program, class or interface declared INTERNAL is
//! named only inside its exact declaring namespace, as an INTERNAL member is, and INTERNAL outside
//! any namespace is an error.

use super::common::*;

/// Every error of `source`, with its message.
fn errors(source: &str) -> Vec<String> {
    let mut db = Database::new();
    let file = FileId(0);
    db.set_source_text(file, source.to_string());
    db.diagnostics(file)
        .iter()
        .filter(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
        .map(|diagnostic| diagnostic.message.to_string())
        .collect()
}

/// IEC 61131-3 Ed.3 §6.9.2, Example 1: the namespace Timers with its INTERNAL function TimeTick.
const TIMERS: &str = "NAMESPACE Timers
FUNCTION INTERNAL TimeTick : DWORD
TimeTick := DWORD#1;
END_FUNCTION
FUNCTION Now : DWORD
Now := TimeTick();
END_FUNCTION
END_NAMESPACE
";

#[test]
fn an_internal_function_is_called_and_returns_inside_its_namespace() {
    // Its own result assignment and a call from another function of Timers are inside the
    // namespace. Catches the member rule applied to a POU, which has no owner, so that both were
    // refused as 'cannot access INTERNAL member' and the function lost its return value.
    assert_eq!(errors(TIMERS), Vec::<String>::new());
}

#[test]
fn an_internal_function_is_refused_outside_its_namespace_qualified_and_through_using() {
    // From the global namespace, `Timers.TimeTick()` and `TimeTick()` through `USING Timers` are
    // both outside Timers. Catches the qualified call, which was never checked, and the call
    // through USING.
    let message = "cannot access INTERNAL 'TimeTick' outside namespace 'Timers'";
    let qualified = format!(
        "{TIMERS}PROGRAM Main\nVAR\n    t : DWORD;\nEND_VAR\nt := Timers.TimeTick();\nEND_PROGRAM\n"
    );
    assert_eq!(errors(&qualified), [message]);
    let imported = format!(
        "{TIMERS}USING Timers;\nPROGRAM Main\nVAR\n    t : DWORD;\nEND_VAR\nt := TimeTick();\nEND_PROGRAM\n"
    );
    assert_eq!(errors(&imported), [message]);
}

#[test]
fn internal_types_serve_inside_their_namespace_and_are_refused_outside() {
    // A function block, a TYPE block, a class and an interface, each INTERNAL in N: the program
    // inside N declares a variable of each; the program outside, naming them `N.<name>`, gets one
    // error for each. Catches a type reference that was never checked, and `TYPE INTERNAL` that
    // left its types public.
    let types = "NAMESPACE N
FUNCTION_BLOCK INTERNAL Fb
END_FUNCTION_BLOCK
TYPE INTERNAL Level : INT; END_TYPE
CLASS INTERNAL C
END_CLASS
INTERFACE INTERNAL I
END_INTERFACE
PROGRAM Inside
VAR
    f : Fb;
    l : Level;
    c : C;
    i : I;
END_VAR
END_PROGRAM
END_NAMESPACE
";
    assert_eq!(errors(types), Vec::<String>::new());
    let outside = format!(
        "{types}PROGRAM Outside\nVAR\n    f : N.Fb;\n    l : N.Level;\n    c : N.C;\n    i : N.I;\nEND_VAR\nEND_PROGRAM\n"
    );
    assert_eq!(
        errors(&outside),
        [
            "cannot use INTERNAL type 'Fb' outside namespace 'N'",
            "cannot use INTERNAL type 'Level' outside namespace 'N'",
            "cannot use INTERNAL type 'C' outside namespace 'N'",
            "cannot use INTERNAL type 'I' outside namespace 'N'",
        ]
    );
}

#[test]
fn internal_is_limited_to_the_exact_declaring_namespace() {
    // Specification 04 keeps INTERNAL to the exact declaring namespace, as for members: a function
    // of the nested namespace Outer.Inner calls Outer's INTERNAL function from another namespace.
    // Catches INTERNAL widened to the namespaces nested in its own.
    let source = "NAMESPACE Outer
FUNCTION INTERNAL Secret : INT
Secret := 1;
END_FUNCTION
NAMESPACE Inner
FUNCTION Peek : INT
Peek := Outer.Secret();
END_FUNCTION
END_NAMESPACE
END_NAMESPACE
";
    assert_eq!(
        errors(source),
        ["cannot access INTERNAL 'Secret' outside namespace 'Outer'"]
    );
}

#[test]
fn internal_outside_any_namespace_is_refused() {
    // §6.9.2: INTERNAL limits an element to its namespace; a function in none has nothing to be
    // limited to. Catches INTERNAL accepted silently on a global POU, and the member message.
    let source = "FUNCTION INTERNAL Lonely : INT\nLonely := 1;\nEND_FUNCTION\n";
    assert_eq!(
        errors(source),
        ["INTERNAL applies only to an element of a namespace, and 'Lonely' is in none (IEC 61131-3 §6.9.2)"]
    );
}
