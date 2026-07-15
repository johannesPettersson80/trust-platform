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

        let (log_tx, log_rx) = mpsc::channel::<DebugLog>();
        self.session.debug_control().set_log_sender(log_tx);
        let (io_tx, io_rx) = mpsc::channel::<IoSnapshot>();
        self.session.debug_control().set_io_sender(io_tx);
        let (stop_tx, stop_rx) = mpsc::channel::<DebugStop>();
        let stop_control = self.session.debug_control();
        stop_control.set_stop_sender(stop_tx);
        let log_writer = Arc::clone(&writer);
        let log_logger = dap_log.clone();
        let log_seq = Arc::clone(&self.next_seq);
        let log_thread = thread::spawn(move || {
            while let Ok(log) = log_rx.recv() {
                let output = if log.message.ends_with('\n') {
                    log.message
                } else {
                    format!("{}\n", log.message)
                };
                let body = OutputEventBody {
                    output,
                    category: Some("console".to_string()),
                    source: None,
                    line: None,
                    column: None,
                };
                let event = Event {
                    seq: log_seq.fetch_add(1, Ordering::Relaxed),
                    message_type: MessageType::Event,
                    event: "output".to_string(),
                    body: Some(body),
                };
                let serialized = match serde_json::to_string(&event) {
                    Ok(serialized) => serialized,
                    Err(_) => continue,
                };
                if let Some(logger) = &log_logger {
                    let _ = write_protocol_log(logger, "->", &serialized);
                }
                if write_message_locked(&log_writer, &serialized).is_err() {
                    break;
                }
            }
        });
        let io_writer = Arc::clone(&writer);
        let io_logger = dap_log.clone();
        let io_seq = Arc::clone(&self.next_seq);
        let io_state_cache = Arc::clone(&self.last_io_state);
        let io_thread = thread::spawn(move || {
            let mut last_sent = Instant::now() - IO_EVENT_MIN_INTERVAL;
            while let Ok(snapshot) = io_rx.recv() {
                let mut latest = snapshot;
                while let Ok(next) = io_rx.try_recv() {
                    latest = next;
                }
                let body = io_state_from_snapshot(latest);
                let mut should_emit = true;
                if let Ok(mut cache) = io_state_cache.lock() {
                    if let Some(previous) = cache.as_ref() {
                        if previous == &body {
                            should_emit = false;
                        }
                    }
                    if should_emit {
                        *cache = Some(body.clone());
                    }
                }
                if !should_emit {
                    continue;
                }
                let elapsed = last_sent.elapsed();
                if elapsed < IO_EVENT_MIN_INTERVAL {
                    thread::sleep(IO_EVENT_MIN_INTERVAL - elapsed);
                }
                let event = Event {
                    seq: io_seq.fetch_add(1, Ordering::Relaxed),
                    message_type: MessageType::Event,
                    event: "stIoState".to_string(),
                    body: Some(body),
                };
                let serialized = match serde_json::to_string(&event) {
                    Ok(serialized) => serialized,
                    Err(_) => continue,
                };
                if let Some(logger) = &io_logger {
                    let _ = write_protocol_log(logger, "->", &serialized);
                }
                if write_message_locked(&io_writer, &serialized).is_err() {
                    break;
                }
                last_sent = Instant::now();
            }
        });
        let stop_thread = StopCoordinator::new(
            self.stop_gate.clone(),
            Arc::clone(&self.pause_expected),
            self.session.debug_control(),
            Arc::clone(&writer),
            dap_log.clone(),
            Arc::clone(&self.next_seq),
        )
        .spawn(stop_rx);

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
                let actions = self.launch_state.pending_actions();
                emit_verbose(
                    self,
                    &writer,
                    &dap_log,
                    format!(
                        "[trust-debug][dap] dispatch: seq={} command={} configured={} pending_launch={} post_launch_actions={:?}",
                        request.seq,
                        request.command,
                        self.launch_state.is_configured(),
                        self.launch_state.has_pending_launch(),
                        actions
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
                let actions = self.launch_state.pending_actions();
                emit_verbose(
                    self,
                    &writer,
                    &dap_log,
                    format!(
                        "[trust-debug][dap] outcome: responses={} events={} should_exit={} configured={} pending_launch={} post_launch_actions={:?}",
                        outcome.responses.len(),
                        outcome.events.len(),
                        outcome.should_exit,
                        self.launch_state.is_configured(),
                        self.launch_state.has_pending_launch(),
                        actions
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
            let actions = self.launch_state.take_actions();
            if actions.pause_after_launch {
                self.pause_expected.store(true, Ordering::SeqCst);
                self.session.debug_control().pause_entry();
            }
            if actions.start_runner_after_launch && self.runner.is_none() {
                self.start_runner();
                let event = self.debug_output_message("[trust-debug] runner started (post-launch)");
                let serialized = serde_json::to_string(&event)
                    .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
                if let Some(logger) = &dap_log {
                    let _ = write_protocol_log(logger, "->", &serialized);
                }
                write_message_locked(&writer, &serialized)?;
            }
            if outcome.should_exit {
                break;
            }
        }

        self.stop_runner();
        self.stop_remote_polling();
        self.session.debug_control().clear_log_sender();
        self.session.debug_control().clear_io_sender();
        self.session.debug_control().clear_stop_sender();
        let _ = log_thread.join();
        let _ = io_thread.join();
        let _ = stop_thread.join();
        Ok(())
    }

}
