impl DebugAdapter {
    #[must_use]
    pub fn new(session: impl DebugRuntime + 'static) -> Self {
        Self {
            session: Box::new(session),
            remote_session: None,
            remote_stop_poller: None,
            remote_breakpoints: Arc::new(Mutex::new(HashMap::new())),
            next_seq: Arc::new(AtomicU32::new(1)),
            coordinate: CoordinateConverter::new(true, true),
            variable_handles: HashMap::new(),
            next_variable_ref: 1,
            watch_cache: HashMap::new(),
            runner: None,
            control_server: None,
            last_io_state: Arc::new(Mutex::new(None)),
            forced_io_addresses: Arc::new(Mutex::new(HashSet::new())),
            launch_state: LaunchState::default(),
            pause_expected: Arc::new(AtomicBool::new(false)),
            stop_gate: StopGate::new(),
            dap_writer: None,
            dap_logger: None,
        }
    }


    pub fn session(&self) -> &dyn DebugRuntime {
        self.session.as_ref()
    }


    pub fn session_mut(&mut self) -> &mut dyn DebugRuntime {
        self.session.as_mut()
    }


    #[must_use]
    pub fn into_session(self) -> Box<dyn DebugRuntime> {
        self.session
    }


    #[must_use]
    pub fn set_breakpoints(&mut self, args: SetBreakpointsArguments) -> SetBreakpointsResponseBody {
        if self.remote_session.is_some() {
            return self.set_breakpoints_remote(args);
        }
        let adjusted = self.to_session_breakpoints(args);
        let response = self.session.set_breakpoints(&adjusted);
        self.to_client_breakpoints(response)
    }

}
