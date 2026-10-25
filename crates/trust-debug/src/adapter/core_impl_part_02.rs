impl DebugAdapter {
    /// Run a blocking stdio loop that processes DAP requests.
    pub fn run_stdio(&mut self) -> io::Result<()> {
        let stdin = io::stdin();
        let mut reader = BufReader::new(stdin.lock());
        let writer = Arc::new(Mutex::new(BufWriter::new(io::stdout())));
        self.dap_writer = Some(writer.clone());

        fn emit_verbose(
            adapter: &DebugAdapter,
            writer: &Arc<Mutex<BufWriter<io::Stdout>>>,
            dap_log: &Option<Arc<Mutex<BufWriter<std::fs::File>>>>,
            message: String,
        ) -> io::Result<()> {
            let event = adapter.debug_output_message(message);
            let serialized = serde_json::to_string(&event)
                .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
            if let Some(logger) = dap_log {
                let _ = write_protocol_log(logger, "->", &serialized);
            }
            write_message_locked(writer, &serialized)
        }

        let dap_log_path = std::env::var("ST_DEBUG_DAP_LOG").ok();
        let dap_log = dap_log_path
            .as_deref()
            .and_then(|path| OpenOptions::new().create(true).append(true).open(path).ok())
            .map(BufWriter::new)
            .map(|writer| Arc::new(Mutex::new(writer)));
        self.dap_logger = dap_log.clone();
        let dap_verbose = env_flag("ST_DEBUG_DAP_VERBOSE");

        let mut announced_verbose = false;

        loop {
            let Some(payload) = read_message(&mut reader)? else {
                if dap_verbose {
                    emit_verbose(
                        self,
                        &writer,
                        &dap_log,
                        "[trust-debug][dap] stdin closed".to_string(),
                    )?;
                }
                break;
            };
            if let Some(logger) = &dap_log {
                let _ = write_protocol_log(logger, "<-", &payload);
            }
            if dap_verbose && !announced_verbose {
                let log_hint = match dap_log_path.as_deref() {
                    Some(path) => {
                        format!("[trust-debug] DAP verbose logging enabled; raw log: {path}")
                    }
                    None => "[trust-debug] DAP verbose logging enabled (set ST_DEBUG_DAP_LOG=/path for raw)".to_string(),
                };
                emit_verbose(self, &writer, &dap_log, log_hint)?;
                announced_verbose = true;
            }
            if dap_verbose {
                emit_verbose(
                    self,
                    &writer,
                    &dap_log,
                    format!(
                        "[trust-debug][dap<-] len={} payload={}",
                        payload.len(),
                        payload
                    ),
                )?;
            }

            let request: Request<Value> = match serde_json::from_str(&payload) {
                Ok(request) => request,
                Err(err) => {
                    if dap_verbose {
                        emit_verbose(
                            self,
                            &writer,
                            &dap_log,
                            format!("[trust-debug][dap] invalid json: {err} payload={payload}"),
                        )?;
                    }
                    continue;
                }
            };

            if dap_verbose {
                emit_verbose(
                    self,
                    &writer,
                    &dap_log,
                    format!(
                        "[trust-debug][dap] dispatch: seq={} command={} configured={} pending_launch={}",
                        request.seq,
                        request.command,
                        self.launch_state.is_configured(),
                        self.launch_state.has_pending_launch(),
                    ),
                )?;
            }
            let command = request.command.clone();
            let mut outcome = self.dispatch_request(request);
            if let Some(mut timeout_outcome) = self.maybe_force_start_after_timeout(&command) {
                outcome.responses.append(&mut timeout_outcome.responses);
                outcome.events.append(&mut timeout_outcome.events);
                outcome.should_exit |= timeout_outcome.should_exit;
            }
            let _stop_gate = outcome.stop_gate.as_ref();
            if dap_verbose {
                emit_verbose(
                    self,
                    &writer,
                    &dap_log,
                    format!(
                        "[trust-debug][dap] outcome: responses={} events={} should_exit={} configured={} pending_launch={}",
                        outcome.responses.len(),
                        outcome.events.len(),
                        outcome.should_exit,
                        self.launch_state.is_configured(),
                        self.launch_state.has_pending_launch(),
                    ),
                )?;
            }
            for response in outcome.responses {
                let serialized = serde_json::to_string(&response)
                    .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
                if let Some(logger) = &dap_log {
                    let _ = write_protocol_log(logger, "->", &serialized);
                }
                if dap_verbose {
                    emit_verbose(
                        self,
                        &writer,
                        &dap_log,
                        format!("[trust-debug][dap->] {serialized}"),
                    )?;
                }
                write_message_locked(&writer, &serialized)?;
            }
            for event in outcome.events {
                let serialized = serde_json::to_string(&event)
                    .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
                if let Some(logger) = &dap_log {
                    let _ = write_protocol_log(logger, "->", &serialized);
                }
                if dap_verbose {
                    emit_verbose(
                        self,
                        &writer,
                        &dap_log,
                        format!("[trust-debug][dap->] {serialized}"),
                    )?;
                }
                write_message_locked(&writer, &serialized)?;
            }
            if outcome.should_exit {
                break;
            }
        }

        self.stop_remote_polling();
        self.stop_simulator();
        Ok(())
    }
}
