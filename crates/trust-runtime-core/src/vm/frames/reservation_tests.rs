use super::*;

fn frame() -> VmFrame {
    VmFrame {
        activation: None,
        pou_id: Some(1),
        return_pc: 0,
        code_start: 0,
        code_end: 0,
        local_ref_start: 0,
        local_ref_count: 0,
        locals: Vec::new(),
        parameter_values_present: Vec::new(),
        runtime_instance: None,
        instance_owner: None,
    }
}

/// Spec 34 MEM-03 and TIME-08: storage reserved at construction holds every frame up to the
/// reservation without moving, and each push is charged exactly the growth that a stack
/// growing on demand is charged, before and after the stack is cleared. Past the reservation
/// the stack grows to the charged capacity, as a stack without a reservation does.
#[test]
fn reserved_frame_storage_keeps_the_charged_growth_of_an_unreserved_stack() {
    let mut unreserved = FrameStack::default();
    let mut reserved = FrameStack::default();
    reserved.reserve_storage(6).unwrap();
    let storage = reserved.frames.as_ptr();
    for round in 0..2 {
        for push in 0..6 {
            assert_eq!(
                reserved.growth_demand().ok(),
                unreserved.growth_demand().ok(),
                "round {round}, push {push}"
            );
            reserved.push(frame()).unwrap();
            unreserved.push(frame()).unwrap();
        }
        assert_eq!(reserved.frames.as_ptr(), storage, "round {round}");
        assert_eq!(reserved.charged_capacity, unreserved.frames.capacity());
        reserved.clear();
        unreserved.clear();
    }
    for push in 0..9 {
        assert_eq!(
            reserved.growth_demand().ok(),
            unreserved.growth_demand().ok(),
            "push {push} past the reservation"
        );
        reserved.push(frame()).unwrap();
        unreserved.push(frame()).unwrap();
    }
    assert_eq!(reserved.charged_capacity, unreserved.charged_capacity);
    assert_eq!(reserved.frames.capacity(), unreserved.frames.capacity());
}
