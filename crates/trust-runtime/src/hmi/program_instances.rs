//! Finding the instances of a function block in the running program: every instance whose type is
//! the block or extends it, at any depth, where it is declared (specification 40, §5 items 1 and 5;
//! specification 39, §18 Phase 5), through the program's symbol and type tables (specification
//! 44 §13.3). An interface, a reference and an in-out member only refer to an instance declared
//! elsewhere and hold none, so each instance is found once.

use crate::runtime::program_access::{Located, ProgramRead, Walk};

/// An instance the walk found.
pub(crate) struct Found {
    pub(crate) path: String,
    pub(crate) at: Located,
    /// The function block instance nearest above it that holds it (specification 39, §13.6,
    /// item 9); none when a program or the globals hold it directly.
    pub(crate) equipment: Option<String>,
}

/// Every instance whose type is `base` or extends it, in declaration order (each program
/// instance's variables, then the globals), with its equipment, at most `limit`. A found
/// instance's own members are not searched.
pub(crate) fn instances_of(program: &dyn ProgramRead, base: &str, limit: usize) -> Vec<Found> {
    let mut found = Vec::new();
    if limit == 0 {
        return found;
    }
    // The instances the walk is inside, outermost first; the innermost holds what it finds.
    let mut holders: Vec<String> = Vec::new();
    program.instances(&mut |path, at| {
        while holders.last().is_some_and(|holder| !inside(path, holder)) {
            holders.pop();
        }
        if program.is_a(&at, base) {
            found.push(Found {
                path: path.to_string(),
                at,
                equipment: holders.last().cloned(),
            });
            return if found.len() >= limit {
                Walk::Stop
            } else {
                Walk::Skip
            };
        }
        holders.push(path.to_string());
        Walk::Descend
    });
    found
}

/// Whether `path` lies inside the instance at `holder`: `Main.P1.Alarm` and `Main.P1.Pumps[2]`
/// inside `Main.P1`.
fn inside(path: &str, holder: &str) -> bool {
    path.strip_prefix(holder)
        .is_some_and(|rest| rest.starts_with('.') || rest.starts_with('['))
}

/// Whether the program declares the function block or class `name`, compared without case: a
/// program without the block has no instance of it to search for (specification 40, §5 item 5).
pub(crate) fn declares_block(program: &dyn ProgramRead, name: &str) -> bool {
    use trust_runtime_core::native::image::{type_kind, TypeRecord};
    let image = program.program_image();
    let Ok(types) = image.table::<TypeRecord>() else {
        return false;
    };
    types.iter().any(|record| {
        matches!(record.kind, type_kind::FUNCTION_BLOCK | type_kind::CLASS)
            && record.name != 0
            && image
                .string(record.name)
                .is_some_and(|declared| declared.eq_ignore_ascii_case(name))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::TestHarness;

    const PROGRAM: &str = r#"
FUNCTION_BLOCK Alarm
VAR_OUTPUT
    Active : BOOL;
END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK LimitAlarm EXTENDS Alarm
END_FUNCTION_BLOCK

FUNCTION_BLOCK Pump
VAR
    Failed : Alarm;
    Overload : LimitAlarm;
END_VAR
    Failed();
    Overload();
END_FUNCTION_BLOCK

INTERFACE IPump
END_INTERFACE

PROGRAM Main
VAR
    High : Alarm;
    Pumps : ARRAY[1..2] OF Pump;
    Lead : REF_TO Pump;
END_VAR
    High();
    Pumps[1]();
    Pumps[2]();
    Lead := REF(Pumps[1]);
END_PROGRAM
"#;

    fn paths(found: &[Found]) -> Vec<(String, Option<String>)> {
        found
            .iter()
            .map(|found| (found.path.clone(), found.equipment.clone()))
            .collect()
    }

    /// Specification 40, §5 items 1 and 5: every alarm, a derived one too, once where it is
    /// declared, with the instance nearest above it that holds it as its equipment; a reference
    /// to an instance finds nothing again. Catches a walk that misses a derived block, lists an
    /// instance again through a reference, names a program as equipment or goes past the limit.
    #[test]
    fn every_instance_of_a_block_or_a_derived_one_is_found_once_with_its_equipment() {
        let mut harness = TestHarness::from_source(PROGRAM).expect("the program compiles");
        harness.cycle();
        let found = harness
            .runtime()
            .with_program_view(|program| paths(&instances_of(program, "Alarm", 100)));
        assert_eq!(
            found,
            [
                ("Main.High".to_string(), None),
                (
                    "Main.Pumps[1].Failed".to_string(),
                    Some("Main.Pumps[1]".to_string())
                ),
                (
                    "Main.Pumps[1].Overload".to_string(),
                    Some("Main.Pumps[1]".to_string())
                ),
                (
                    "Main.Pumps[2].Failed".to_string(),
                    Some("Main.Pumps[2]".to_string())
                ),
                (
                    "Main.Pumps[2].Overload".to_string(),
                    Some("Main.Pumps[2]".to_string())
                ),
            ]
        );
        let first_two = harness
            .runtime()
            .with_program_view(|program| paths(&instances_of(program, "Alarm", 2)));
        assert_eq!(first_two.len(), 2);
        harness.runtime().with_program_view(|program| {
            assert!(declares_block(program, "limitalarm"));
            assert!(
                !declares_block(program, "IPump"),
                "an interface is no block"
            );
            assert!(!declares_block(program, "Hmi.Display"));
        });
    }
}
