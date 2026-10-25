#[test]
fn capabilities_answers_the_hosted_descriptor() {
    // Specification 34 §5.1.1 (HOST-02) and specification 11, the runtime device capabilities
    // contract: a computer runs, on an x86-64 or AArch64 CPU, compiled program images, 44.1
    // (specification 44 §4.4, §8.8), and no bytecode, whose engine is gone (§15),
    // cooperative-resource-v1 and numeric contract 1, has unqualified timing evidence and the wall
    // clock, has no maximum for any limit, since no bytecode is prepared (`null`, since a JSON
    // number that large is rounded), keeps 1,000 events by default and admits at most 1,000,000,
    // has no application slot and no maximum of memory for programs (specification 34 MEM-12), and
    // starts as a new process, so its last reset is null. The wrong behaviours it catches: a
    // computer that answers a chip's descriptor (200 events, a slot, a memory for programs); a
    // computer of those CPUs that does not offer program images, or still offers bytecode; a
    // maximum left from the bytecode engine or a missing wall-clock capability; an unbounded limit
    // written as a number; and a last reset reported on a computer.
    let state = hmi_test_state(
        r#"
PROGRAM Main
END_PROGRAM
"#,
    );
    let response = handle_request_value(json!({"id": 9301, "type": "capabilities"}), &state, None);
    assert!(
        response.ok,
        "capabilities should answer: {:?}",
        response.error
    );
    let versions = if cfg!(any(target_arch = "x86_64", target_arch = "aarch64")) {
        json!(["44.1"])
    } else {
        json!([])
    };
    assert_eq!(
        response.result,
        Some(json!({
            "program_versions": versions,
            "scheduling_profiles": ["cooperative-resource-v1"],
            "numeric_contracts": ["1"],
            "timing_evidence": "unqualified",
            "capabilities": ["wall_clock"],
            "maxima": {
                "max_artifact_bytes": null,
                "max_preparation_bytes": null,
                "max_preparation_work": null,
                "validation.max_scratch_bytes": null,
                "validation.max_work": null,
                "max_construction_values": null,
                "max_construction_bytes": null,
                "max_process_image_bytes": null,
                "max_work": null,
                "max_construction_work": null,
                "max_call_depth": null,
            },
            "event_store_default": 1_000,
            "event_store_max": 1_000_000,
            "application_slot_bytes": null,
            "program_memory_bytes": null,
            "last_reset": null,
        })),
        "a computer answers the hosted descriptor with no last reset"
    );
}
