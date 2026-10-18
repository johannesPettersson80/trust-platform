use super::*;

impl VariableStorage {
    pub fn push_frame(&mut self, owner: impl Into<SmolStr>) -> FrameId {
        let id = FrameId(self.activations.next_frame_id);
        self.activations.next_frame_id += 1;
        self.activations.frames.push(LocalFrame {
            id,
            owner: owner.into(),
            variables: IndexMap::default(),
            return_value: None,
            instance_id: None,
        });
        id
    }

    pub fn push_frame_with_instance(
        &mut self,
        owner: impl Into<SmolStr>,
        instance_id: InstanceId,
    ) -> FrameId {
        let id = FrameId(self.activations.next_frame_id);
        self.activations.next_frame_id += 1;
        self.activations.frames.push(LocalFrame {
            id,
            owner: owner.into(),
            variables: IndexMap::default(),
            return_value: None,
            instance_id: Some(instance_id),
        });
        id
    }

    pub fn pop_frame(&mut self) -> Option<LocalFrame> {
        self.activations.frames.pop()
    }

    pub fn remove_frame(&mut self, frame_id: FrameId) -> Option<LocalFrame> {
        let idx = self
            .activations
            .frames
            .iter()
            .position(|frame| frame.id == frame_id)?;
        Some(self.activations.frames.remove(idx))
    }

    #[must_use]
    pub fn frames(&self) -> &[LocalFrame] {
        &self.activations.frames
    }

    #[must_use]
    pub fn current_frame(&self) -> Option<&LocalFrame> {
        self.activations.frames.last()
    }

    pub fn current_frame_mut(&mut self) -> Option<&mut LocalFrame> {
        self.activations.frames.last_mut()
    }

    pub fn set_local(&mut self, name: impl Into<SmolStr>, value: Value) -> bool {
        if let Some(frame) = self.current_frame_mut() {
            frame.variables.insert(name.into(), value);
            true
        } else {
            false
        }
    }

    #[must_use]
    pub fn get_local(&self, name: &str) -> Option<&Value> {
        self.current_frame()
            .and_then(|frame| frame.variables.get(name))
    }

    pub fn clear_locals(&mut self) {
        if let Some(frame) = self.current_frame_mut() {
            frame.variables.clear();
        }
    }

    pub fn clear_frames(&mut self) {
        self.activations.frames.clear();
        self.activations.next_frame_id = 0;
    }

    /// Temporarily treat the provided frame as the current frame.
    pub fn with_frame<T>(
        &mut self,
        frame_id: FrameId,
        f: impl FnOnce(&mut Self) -> T,
    ) -> Option<T> {
        let idx = self
            .activations
            .frames
            .iter()
            .position(|frame| frame.id == frame_id)?;
        if idx + 1 == self.activations.frames.len() {
            return Some(f(self));
        }

        let frame = self.activations.frames.remove(idx);
        self.activations.frames.push(frame);
        let result = f(self);
        let frame = self
            .activations
            .frames
            .pop()
            .expect("frame stack empty after eval");
        self.activations.frames.insert(idx, frame);
        Some(result)
    }
}
