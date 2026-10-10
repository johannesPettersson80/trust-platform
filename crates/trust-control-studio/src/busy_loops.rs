//! One operation per loop at a time in one engine process (specification 35, A8.5): a request that works on a loop claims the
//! loop first, and the claim ends with the work. Two requests of one engine therefore never run on one loop together, even
//! before either has journaled anything. A request of another process, such as the command line beside the extension's
//! engine, is not seen here; only the journal is shared between processes.

use std::collections::BTreeMap;
use std::sync::Arc;

use parking_lot::Mutex;
use trust_control::failure::Failure;
use trust_control::identification::connected_run::loop_busy_refusal;

/// A loop by its project and its identifier.
type LoopKey = (String, String);

/// The loops that operations of this engine are working on, each with the name of the operation that holds it.
#[derive(Debug, Clone, Default)]
pub struct LoopClaims {
    busy: Arc<Mutex<BTreeMap<LoopKey, String>>>,
}

impl LoopClaims {
    /// Claims the loop `loop_id` of the project `project_id` for the operation `operation`, which a refusal names. The check
    /// and the claim are one locked step, so two requests cannot both claim the loop.
    ///
    /// # Errors
    /// An operation of this engine is working on the loop: the request waits for it to end, and the refusal names that
    /// operation (`loop_busy_refusal`).
    pub fn claim(
        &self,
        project_id: &str,
        loop_id: &str,
        operation: &str,
    ) -> Result<LoopClaim, Failure> {
        let key = (project_id.to_string(), loop_id.to_string());
        let mut busy = self.busy.lock();
        if let Some(holder) = busy.get(&key) {
            return Err(loop_busy_refusal(holder));
        }
        busy.insert(key.clone(), operation.to_string());
        Ok(LoopClaim {
            busy: Arc::clone(&self.busy),
            key,
        })
    }
}

/// A loop claimed for one operation. Dropping it ends the claim, also when the work returns early or panics.
#[derive(Debug)]
#[must_use = "the claim ends when it is dropped"]
pub struct LoopClaim {
    busy: Arc<Mutex<BTreeMap<LoopKey, String>>>,
    key: LoopKey,
}

impl Drop for LoopClaim {
    fn drop(&mut self) {
        self.busy.lock().remove(&self.key);
    }
}

#[cfg(test)]
mod tests {
    use trust_control::failure::NextAction;

    use super::*;

    #[test]
    fn a_claimed_loop_is_refused_until_its_claim_is_dropped_and_the_refusal_names_its_holder() {
        let claims = LoopClaims::default();
        let first = claims
            .claim("project", "tank", "Identify plant")
            .expect("a free loop");
        let Err(refusal) = claims.claim("project", "tank", "Reset") else {
            panic!("a claimed loop is refused");
        };
        assert_eq!(refusal, loop_busy_refusal("Identify plant"));
        assert_eq!(
            refusal.message,
            "Identify plant is running in this engine on this loop."
        );
        assert_eq!(refusal.next_action, NextAction::Wait);
        // Another loop, and the same loop identifier in another project, are other loops.
        let _boiler = claims.claim("project", "boiler", "Reset").expect("free");
        let _elsewhere = claims.claim("other", "tank", "Reset").expect("free");
        drop(first);
        let _again = claims
            .claim("project", "tank", "Reset")
            .expect("free again");
    }
}
