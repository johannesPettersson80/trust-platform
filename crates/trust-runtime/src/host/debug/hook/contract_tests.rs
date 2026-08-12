use crate::memory::VariableStorage;
use crate::value::Value;
use trust_hir::types::TypeRegistry;

use super::*;

#[derive(Default)]
struct RecordingHook {
    calls: Vec<(Option<SourceLocation>, u32)>,
}

impl DebugHook for RecordingHook {
    fn on_statement(&mut self, location: Option<&SourceLocation>, call_depth: u32) {
        self.calls.push((location.copied(), call_depth));
    }
}

fn context<'a>(
    storage: &'a mut VariableStorage,
    registry: &'a TypeRegistry,
) -> DebugRuntimeContext<'a> {
    DebugRuntimeContext {
        storage,
        registry,
        stdlib: None,
        profile: DateTimeProfile::default(),
        current_instance: None,
        now: Duration::from_millis(17),
    }
}

#[test]
fn default_context_callback_delegates_exactly_once_to_plain_callback() {
    let mut storage = VariableStorage::new();
    let registry = TypeRegistry::new();
    let mut ctx = context(&mut storage, &registry);
    let location = SourceLocation::new(7, 10, 20);
    let mut hook = RecordingHook::default();

    hook.on_statement_with_context(&mut ctx, Some(&location), 3);
    assert_eq!(hook.calls, vec![(Some(location), 3)]);
}

#[test]
fn default_context_callback_preserves_absent_location() {
    let mut storage = VariableStorage::new();
    let registry = TypeRegistry::new();
    let mut ctx = context(&mut storage, &registry);
    let mut hook = RecordingHook::default();

    hook.on_statement_with_context(&mut ctx, None, u32::MAX);
    assert_eq!(hook.calls, vec![(None, u32::MAX)]);
}

#[test]
fn noop_hook_accepts_plain_and_context_callbacks_without_storage_mutation() {
    let mut storage = VariableStorage::new();
    storage.set_global("value", Value::DInt(7));
    let registry = TypeRegistry::new();
    let location = SourceLocation::new(1, 0, 1);
    let mut hook = NoopDebugHook;

    hook.on_statement(Some(&location), 1);
    let mut ctx = context(&mut storage, &registry);
    hook.on_statement_with_context(&mut ctx, Some(&location), 2);
    assert_eq!(ctx.storage.get_global("value"), Some(&Value::DInt(7)));
}

#[test]
fn noop_hook_is_copyable_and_usable_through_trait_object() {
    let first = NoopDebugHook;
    let mut second = first;
    let hook: &mut dyn DebugHook = &mut second;
    hook.on_statement(None, 0);
}

#[test]
fn runtime_context_preserves_supplied_storage_registry_profile_and_time() {
    let mut storage = VariableStorage::new();
    storage.set_global("value", Value::DInt(9));
    let registry = TypeRegistry::new();
    let ctx = context(&mut storage, &registry);

    assert_eq!(ctx.storage.get_global("value"), Some(&Value::DInt(9)));
    assert!(std::ptr::eq(ctx.registry, &registry));
    assert!(ctx.stdlib.is_none());
    assert_eq!(ctx.current_instance, None);
    assert_eq!(ctx.now, Duration::from_millis(17));
}
