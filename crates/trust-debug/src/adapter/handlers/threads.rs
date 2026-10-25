//! Thread requests.
//! - handle_threads: enumerate runtime tasks

use serde_json::Value;

use crate::protocol::{Request, Thread, ThreadsResponseBody};

use super::super::{DebugAdapter, DispatchOutcome};

impl DebugAdapter {
    /// The threads of the program: one per task of the project's declarations, numbered as the
    /// runtime numbers them, and one more for the programs outside every task (specification 13,
    /// "Multi-Task Thread Model"); one thread when the project's tasks are unknown.
    pub(in crate::adapter) fn projected_threads(&self) -> Vec<Thread> {
        let metadata = self.session.metadata();
        let tasks = metadata.tasks();
        if tasks.is_empty() {
            return vec![Thread {
                id: 1,
                name: "MainTask".to_string(),
            }];
        }
        let mut threads: Vec<Thread> = Vec::new();
        for (index, task) in tasks.iter().enumerate() {
            let id = metadata
                .task_thread_id(&task.name)
                .unwrap_or(index as u32 + 1);
            if threads.iter().all(|thread| thread.id != id) {
                threads.push(Thread {
                    id,
                    name: task.name.to_string(),
                });
            }
        }
        if metadata.has_background_programs() {
            let fallback = threads
                .iter()
                .map(|thread| thread.id)
                .max()
                .unwrap_or(0)
                .saturating_add(1);
            threads.push(Thread {
                id: metadata.background_thread_id().unwrap_or(fallback),
                name: "Background".to_string(),
            });
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
