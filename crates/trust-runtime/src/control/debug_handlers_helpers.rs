fn debug_stop_to_json(
    stop: crate::debug::DebugStop,
    state: &ControlState,
) -> Option<serde_json::Value> {
    // The stop names its place only when its file's text is loaded and the file is registered.
    let place = stop.location.and_then(|location| {
        let text = state.sources.source_text(location.file_id)?;
        let source = state
            .sources
            .files()
            .iter()
            .find(|file| file.id == location.file_id)?;
        let (line, column) = location_to_line_col(text, &location);
        Some((
            location.file_id,
            line,
            column,
            source.path.to_string_lossy().to_string(),
        ))
    });
    Some(replies::stop(
        stop.reason,
        stop.thread_id,
        stop.breakpoint_generation,
        place
            .as_ref()
            .map(|(file_id, line, column, path)| StopPlace {
                file_id: *file_id,
                line: *line,
                column: *column,
                path: path.as_str(),
            }),
    ))
}

fn location_to_source(
    location: &crate::debug::SourceLocation,
    state: &ControlState,
) -> Option<(DebugSource, u32, u32)> {
    let source_text = state.sources.source_text(location.file_id)?;
    let (line, column) = location_to_line_col(source_text, location);
    let source = state
        .sources
        .files()
        .iter()
        .find(|file| file.id == location.file_id)?;
    let name = source
        .path
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.to_string());
    let path = Some(source.path.to_string_lossy().to_string());
    Some((DebugSource { name, path }, line, column))
}

fn location_to_stack_frame(
    frame_id: u32,
    frame_name: &str,
    location: &crate::debug::SourceLocation,
    state: &ControlState,
) -> Option<serde_json::Value> {
    let (source, line, column) = location_to_source(location, state)?;
    Some(replies::stack_frame(
        frame_id, frame_name, &source, line, column,
    ))
}

fn evaluate_with_snapshot(
    expr: &crate::program_model::Expr,
    registry: &trust_hir::types::TypeRegistry,
    frame_id: Option<crate::memory::FrameId>,
    snapshot: &crate::debug::DebugSnapshot,
    _using: &[smol_str::SmolStr],
    state: &ControlState,
) -> Result<Value, RuntimeError> {
    let metadata = state
        .metadata
        .lock()
        .map_err(|_| RuntimeError::ControlError("metadata unavailable".into()))?;
    let profile = metadata.profile();
    let mut storage = snapshot.storage.clone();
    let eval = |storage: &mut crate::memory::VariableStorage,
                instance_id: Option<crate::memory::InstanceId>|
     -> Result<Value, RuntimeError> {
        crate::helper_eval::eval_storage_expr_with_abi(
            storage,
            registry,
            snapshot.storage_abi,
            &profile,
            instance_id,
            Some(metadata.stdlib()),
            expr,
        )
    };

    if let Some(frame_id) = frame_id {
        storage
            .with_frame(frame_id, |storage| {
                let instance_id = storage.current_frame().and_then(|frame| frame.instance_id);
                eval(storage, instance_id)
            })
            .ok_or(RuntimeError::InvalidFrame(frame_id.0))?
    } else {
        eval(&mut storage, None)
    }
}

#[cfg(test)]
mod instance_order_tests {
    use super::*;
    use crate::debug::{DebugVariableHandles, VariableHandle};
    use crate::memory::VariableStorage;
    use trust_runtime_services::debug::paging::Page;

    #[test]
    fn instances_list_in_one_order_on_every_runtime() {
        // Specification 11 §6.13.3: the Instances scope lists its instances in ascending instance
        // identifier on every runtime. A computer's storage keeps its instances in a hash map, which
        // does not list twelve identifiers in ascending order, so this test reads the computer's own
        // storage type. Twelve instances take the identifiers 0 to 11. Wrong behaviour caught: a listing
        // in the hash map's order, and an instance that is dropped from the list.
        let mut storage = VariableStorage::default();
        for _ in 0..12 {
            storage
                .try_create_instance("Fb")
                .expect("an instance identity");
        }
        let read = ReadState {
            storage: &storage,
            frames: Vec::new(),
        };
        let mut handles = DebugVariableHandles::new();

        let listed = reads::variables(
            &read,
            &VariableHandle::Instances,
            None,
            &mut handles,
            Page::default(),
        )
        .expect("the instances list");

        let names = listed
            .iter()
            .map(|variable| variable.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            [
                "Fb#0", "Fb#1", "Fb#2", "Fb#3", "Fb#4", "Fb#5", "Fb#6", "Fb#7", "Fb#8", "Fb#9",
                "Fb#10", "Fb#11",
            ]
        );
    }
}
