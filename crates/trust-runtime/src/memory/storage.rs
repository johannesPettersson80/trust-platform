use super::*;

impl VariableStorage {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_global(&mut self, name: impl Into<SmolStr>, value: Value) {
        self.globals.insert(name.into(), value);
    }

    #[must_use]
    pub fn globals(&self) -> &IndexMap<SmolStr, Value> {
        &self.globals
    }

    #[must_use]
    pub fn get_global(&self, name: &str) -> Option<&Value> {
        self.globals.get(name)
    }

    pub fn set_retain(&mut self, name: impl Into<SmolStr>, value: Value) {
        self.retain.insert(name.into(), value);
    }

    #[must_use]
    pub fn retain(&self) -> &IndexMap<SmolStr, Value> {
        &self.retain
    }

    #[must_use]
    pub fn get_retain(&self, name: &str) -> Option<&Value> {
        self.retain.get(name)
    }

    pub fn reset_runtime_values(&mut self, reset_instance_sequence: bool) {
        self.globals.clear();
        self.frames.clear();
        self.instances.clear();
        self.next_frame_id = 0;
        if reset_instance_sequence {
            self.next_instance_id = 0;
        }
        self.instance_field_offsets
            .write()
            .expect("instance_field_offsets poisoned")
            .clear();
        self.recursive_instance_field_resolutions
            .write()
            .expect("recursive_instance_field_resolutions poisoned")
            .clear();
    }
}
