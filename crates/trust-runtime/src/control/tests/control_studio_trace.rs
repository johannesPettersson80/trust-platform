#[test]
fn control_studios_writes_are_recorded_with_its_source() {
    // Specification 48 RU-59 and Appendix C, C.6: a cycle trace keeps every change from outside
    // with its source; Control Studio's (3) experiment writes, a variable's and an address's,
    // are applied at the cycle's start, its variable writes before the Live Values (specification
    // 35 A3.5). Worked by hand, in a recording of one cycle: Control Studio writes main.kp, which
    // the trace names as the debugger does, Main.Kp (specification 48 Appendix C, C.6), 4.0
    // (LREAL 0x4010000000000000, little-endian 00 00 00 00 00 00 10 40); the control link's
    // `io.write` writes %ML0 11.0 (0x4026000000000000), then Control Studio's address write %ML0
    // 12.5 (0x4029000000000000), each as the LWORD that an LREAL bound at an L-size address is in
    // the process image (`coerce_to_io`). Catches Control Studio's variable write not recorded, its
    // address write recorded as the control link's, and the control link's as Control Studio's.
    use trust_runtime_core::trace::{decode, Change, ChangeKind, ChangeSource, IoArea};
    use trust_runtime_services::trace::requests::{RecordRequest, TraceName, TraceState, Until};

    let (state, mut harness) = transfer_fixture();
    let traces = crate::runtime::new_traces();
    let request = RecordRequest {
        until: Until::Cycles(1),
        task: None,
    };
    harness
        .runtime_mut()
        .start_trace(&request, (&SmolStr::new("studio"), "test"), &traces)
        .expect("the recording starts");
    let program = state.debug.published_program().expect("a runtime snapshot");
    let at = crate::runtime::ProgramRead::locate(&*program, "Main.Kp").expect("Kp's place");
    state.debug.enqueue_control_write(
        crate::debug::ControlWrite {
            at,
            generation: state.debug.generation(),
            label: "main.kp".into(),
            value: Value::LReal(4.0),
        },
        state.debug.runtime_epoch(),
    );
    let marker = crate::io::IoAddress::parse("%ML0").expect("an address");
    state
        .debug
        .try_enqueue_io_write(marker.clone(), Value::LWord(11.0_f64.to_bits()))
        .expect("the control link's write is queued");
    state
        .debug
        .enqueue_io_write(marker, Value::LWord(12.5_f64.to_bits()));
    assert!(harness.cycle().errors.is_empty());
    assert_eq!(studio_value(&harness, "Main.Kp"), Value::LReal(4.0));

    let slot = crate::runtime::lock_traces(&traces);
    let TraceState::Ended { bytes, .. } = slot.state(TraceName::Id(1)) else {
        panic!("the recording of one cycle has ended");
    };
    let mut file = vec![0; bytes];
    assert_eq!(slot.part(TraceName::Id(1), 0, &mut file), Some(bytes));
    let trace = decode(&file).expect("a trace").trace;
    let memory = |source, last| Change {
        source,
        kind: ChangeKind::AddressWrite {
            area: IoArea::Memory,
            bit: None,
            offset: 0,
            bytes: vec![0, 0, 0, 0, 0, 0, last, 0x40],
        },
    };
    assert_eq!(
        trace.cycles[0].changes,
        vec![
            Change {
                source: ChangeSource::ControlStudio,
                kind: ChangeKind::VariableWrite {
                    path: "Main.Kp".into(),
                    type_name: "LREAL".into(),
                    value: vec![0, 0, 0, 0, 0, 0, 0x10, 0x40],
                },
            },
            memory(ChangeSource::ControlLink, 0x26),
            memory(ChangeSource::ControlStudio, 0x29),
        ]
    );
}
