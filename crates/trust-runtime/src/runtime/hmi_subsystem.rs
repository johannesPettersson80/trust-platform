//! The program's `Hmi.Display` calls (specification 39, §18 Phase 5): after each task in which the
//! program called the block, the runtime serves the HMI exchange, so the screens follow the
//! interval and priority of the task the user chose for them.

use std::sync::Arc;

use rustc_hash::FxHashMap;

use crate::hmi::exchange::HmiExchange;
use crate::hmi::instance_walk::instances_of;
use crate::hmi::member_paths::find_ignoring_case;
use crate::memory::InstanceId;
use crate::value::Value;

use super::core::Runtime;

/// The block a program calls to serve its screens.
pub(crate) const DISPLAY_BLOCK: &str = "Hmi.Display";
/// Most blocks followed.
const MAX_DISPLAYS: usize = 16;
/// How often the program's blocks are found again, for an online change that adds one.
const WALK_PERIOD_MS: i64 = 1_000;

/// The exchange and the program's `Hmi.Display` blocks.
#[derive(Default)]
pub(super) struct HmiSubsystem {
    exchange: Option<Arc<HmiExchange>>,
    displays: Vec<InstanceId>,
    /// Each block's call count when last looked at.
    seen: FxHashMap<InstanceId, u64>,
    walked_ms: Option<i64>,
    /// When each link last exchanged data (specification 42, §5 item 4).
    pub(super) links: super::hmi_stale::LinkWatch,
}

impl std::fmt::Debug for HmiSubsystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HmiSubsystem")
            .field("exchange", &self.exchange.is_some())
            .field("displays", &self.displays.len())
            .finish()
    }
}

impl HmiSubsystem {
    pub(super) fn new() -> Self {
        Self::default()
    }
}

impl Runtime {
    /// Shares the HMI exchange with the web server.
    pub fn set_hmi_exchange(&mut self, exchange: Arc<HmiExchange>) {
        exchange.methods.invalidate();
        self.hmi.exchange = Some(exchange);
    }

    /// The HMI exchange, once shared.
    #[must_use]
    pub fn hmi_exchange(&self) -> Option<Arc<HmiExchange>> {
        self.hmi.exchange.clone()
    }

    /// After a task: serves the exchange when the program called `Hmi.Display` in it. A program
    /// without the block is not searched.
    pub(super) fn serve_hmi_after_task(&mut self) -> Result<(), crate::error::RuntimeError> {
        let Some(exchange) = self.hmi.exchange.clone() else {
            return Ok(());
        };
        if find_ignoring_case(&self.function_blocks, DISPLAY_BLOCK).is_none() {
            return Ok(());
        }
        self.find_displays();
        let mut called = false;
        for index in 0..self.hmi.displays.len() {
            let id = self.hmi.displays[index];
            let calls = match self
                .storage
                .get_instance(id)
                .and_then(|instance| find_ignoring_case(&instance.variables, "Calls"))
            {
                Some(Value::UDInt(calls)) => u64::from(*calls),
                _ => continue,
            };
            let last = self.hmi.seen.insert(id, calls);
            if last.map_or(calls > 0, |last| last != calls) {
                called = true;
            }
        }
        if called {
            let silent = self.silent_inputs();
            exchange.methods.serve(self)?;
            exchange.serve(self, &silent);
        }
        Ok(())
    }

    /// Finds the blocks once a second, and at once when a known one is gone (an online change).
    fn find_displays(&mut self) {
        let now_ms = self.current_time.as_millis();
        let gone = self
            .hmi
            .displays
            .iter()
            .any(|id| self.storage.get_instance(*id).is_none());
        let due = self
            .hmi
            .walked_ms
            .is_none_or(|walked| now_ms < walked || now_ms - walked >= WALK_PERIOD_MS);
        if !(gone || due) {
            return;
        }
        let programs = &self.programs;
        let is_program = |name: &str| find_ignoring_case(programs, name).is_some();
        let holdings = crate::hmi::instance_holdings::Holdings::new(
            &self.function_blocks,
            &self.classes,
            &self.programs,
            &self.registry,
        );
        self.hmi.displays = instances_of(
            &self.storage,
            &holdings,
            DISPLAY_BLOCK,
            MAX_DISPLAYS,
            &is_program,
        )
        .into_iter()
        .map(|found| found.id)
        .collect();
        self.hmi.walked_ms = Some(now_ms);
        let displays = self.hmi.displays.clone();
        self.hmi.seen.retain(|id, _| displays.contains(id));
    }
}
