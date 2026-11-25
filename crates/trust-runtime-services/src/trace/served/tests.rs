//! A trace served part by part is, byte for byte, the file the recorder writes (specification 48
//! RU-59, "Memory"; Appendix C), and it names the variables once, when the recording ends.

use alloc::string::String;
use alloc::vec::Vec;

use trust_runtime_core::deployment::sha256;
use trust_runtime_core::trace::{decode, ChangeRef, ChangeSource, TraceEnd};

use super::super::requests::TraceState;
use super::super::tests::{identity, image, ms, read_in_parts, sizes};
use super::super::{Begin, Budget, Goal, Recorder};

/// The debugger's names: three of the paths the recording keeps are named, `tank.level` is not.
fn debugger(path: &str) -> Option<String> {
    match path {
        "main.limit" => Some("Main.limit".into()),
        "main.x" => Some("Main.x".into()),
        "main.heating" => Some("Main.Heating".into()),
        _ => None,
    }
}

/// An ended recording of three cycles at 0, 10 and 20 ms: a start force of `main.limit`; in the
/// second cycle a screen's write of `main.x`, a method call on `main.heating` and a plant's write of
/// `tank.level`; in the third the control link's write of `main.x` again; each cycle with an
/// activation, a wall-clock reading and both images.
fn recording() -> Recorder {
    let mut recorder = Recorder::arm(Goal::Cycles(3), sizes(), Budget::COMPUTER).unwrap();
    for at in [0, 10, 20] {
        let begun = recorder.begin_cycle(ms(at), 7 + u64::try_from(at).unwrap());
        if at == 0 {
            assert_eq!(begun, Begin::First);
            recorder.start_force(
                ChangeSource::ControlLink,
                &ChangeRef::VariableForce {
                    path: "main.limit",
                    type_name: "REAL",
                    value: &21.5f32.to_le_bytes(),
                },
            );
        }
        if at == 10 {
            let write = |path| ChangeRef::VariableWrite {
                path,
                type_name: "INT",
                value: &[0x2A, 0],
            };
            recorder.change(ChangeSource::Screen, &write("main.x"));
            recorder.change(
                ChangeSource::ControlLink,
                &ChangeRef::MethodCall {
                    instance: "main.heating",
                    interface: "ISequence",
                    method: "Start",
                    inputs: &[("Setpoint", "21.5")],
                },
            );
            recorder.change(ChangeSource::Plant, &write("tank.level"));
        }
        if at == 20 {
            recorder.change(
                ChangeSource::ControlLink,
                &ChangeRef::VariableWrite {
                    path: "main.x",
                    type_name: "INT",
                    value: &[0x2B, 0],
                },
            );
        }
        recorder.activation(0, ms(at).as_nanos());
        recorder.wall_clock(1_795_000_000_000 + at);
        recorder.inputs(&[u8::try_from(at).unwrap(), 1]);
        recorder.outputs(&[u8::try_from(at).unwrap(), 2, 3, 4]);
        recorder.end_cycle(None);
    }
    assert_eq!(recorder.end(), Some(TraceEnd::Complete));
    recorder
}

#[test]
fn a_served_trace_is_byte_for_byte_the_file_the_recorder_writes() {
    // RU-59, "Memory": a chip serves a trace part by part from its records, never the whole file,
    // and the file it serves is the recorder's (Appendix C). Worked by hand: the recording above,
    // read in parts of 1, 7, 8, 31, 64 and 500 bytes and in one part as long as the file, gives
    // exactly the bytes of `file_named` with the same names, its length and its SHA-256 in the
    // state; a window across the header, the section table and the first sections is that slice;
    // a part from the file's end or past it is empty. Catches a section misplaced or misnamed,
    // padding not zeroed, an entry or header CRC that differs, a part that runs past the window or
    // stops short, and a SHA-256 of another file.
    let mut recorder = recording();
    let file = recorder
        .file_named(&identity(), &[], &image(), &mut debugger)
        .expect("an ended recording");
    let served = recorder
        .served(&identity(), &[], image(), &mut debugger)
        .expect("an ended recording");
    assert_eq!(served.len(), file.len());
    assert_eq!(
        served.state(),
        TraceState::Ended {
            reason: TraceEnd::Complete,
            cycles: 3,
            bytes: file.len(),
            sha256: sha256(&file),
        }
    );
    for part in [1, 7, 8, 31, 64, 500, file.len()] {
        assert_eq!(read_in_parts(&served, part), file, "parts of {part} bytes");
    }
    let mut window = alloc::vec![0xEE; 300];
    assert_eq!(served.part(40, &mut window), 300);
    assert_eq!(window[..], file[40..340]);
    assert_eq!(served.part(file.len(), &mut window), 0);
    assert_eq!(served.part(file.len() + 5, &mut window), 0);
    let decoded = decode(&read_in_parts(&served, 64)).expect("the served file decodes");
    assert_eq!(decoded.damaged_cycle, None);
    assert_eq!(decoded.trace.cycles.len(), 3);
}

#[test]
fn a_recording_is_served_once_it_has_ended_and_named_once() {
    // A trace is served once its recording has ended, as `trace.read` serves only an ended
    // trace's file; until then the recorder stays as it was. The names are taken once, when it
    // ends: the debugger is asked once for each of the four paths, however often a path occurs
    // and however many parts are read later, so that a program installed after the recording
    // cannot rename them. Catches a recording served while it still records, a name asked again
    // for each record or part, and a part read that needs the program.
    let mut unended = Recorder::arm(Goal::Cycles(2), sizes(), Budget::COMPUTER).unwrap();
    unended.begin_cycle(ms(0), 1);
    unended.inputs(&[0, 0]);
    unended.outputs(&[0; 4]);
    unended.end_cycle(None);
    assert!(unended
        .served(&identity(), &[], image(), &mut debugger)
        .is_none());
    assert_eq!(unended.end(), None);
    assert_eq!(unended.cycles(), 1);

    let mut asked: Vec<String> = Vec::new();
    let served = recording()
        .served(&identity(), &[], image(), &mut |path| {
            asked.push(String::from(path));
            debugger(path)
        })
        .expect("an ended recording");
    asked.sort();
    assert_eq!(
        asked,
        ["main.heating", "main.limit", "main.x", "tank.level"]
    );
    let _ = read_in_parts(&served, 16);
}
