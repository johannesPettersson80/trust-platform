//! Thread requests.
//! - handle_threads: enumerate runtime tasks

use serde_json::Value;

use crate::protocol::{Request, Thread, ThreadsResponseBody};

use super::super::{DebugAdapter, DispatchOutcome};

impl DebugAdapter {
    pub(in crate::adapter) fn projected_threads(&self) -> Vec<Thread> {
        if self.remote_session.is_some() {
            return vec![Thread {
                id: 1,
                name: "MainTask".to_string(),
            }];
        }

        let tasks = self.session.metadata().tasks();
        let mut threads = Vec::new();
        if tasks.is_empty() {
            threads.push(Thread {
                id: 1,
                name: "Main".to_string(),
            });
        } else {
            threads.extend(tasks.iter().enumerate().map(|(idx, task)| {
                let id = self
                    .session
                    .metadata()
                    .task_thread_id(&task.name)
                    .unwrap_or(idx as u32 + 1);
                Thread {
                    id,
                    name: task.name.to_string(),
                }
            }));
            if self.session.metadata().has_background_programs() {
                let fallback = threads
                    .iter()
                    .map(|thread| thread.id)
                    .max()
                    .unwrap_or(0)
                    .saturating_add(1);
                let id = self
                    .session
                    .metadata()
                    .background_thread_id()
                    .unwrap_or(fallback);
                threads.push(Thread {
                    id,
                    name: "Background".to_string(),
                });
            }
        }
        threads
    }

    pub(in crate::adapter) fn handle_threads(
        &mut self,
        request: Request<Value>,
    ) -> DispatchOutcome {
        let body = ThreadsResponseBody {
            threads: self.projected_threads(),
        };
        DispatchOutcome {
            responses: vec![self.ok_response(&request, Some(body))],
            ..DispatchOutcome::default()
        }
    }
}
