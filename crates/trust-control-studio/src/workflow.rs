//! The runtime workflow of Identify (specification 35, A8.5, B3.4 and E11.4): the campaign's loop, the reconciliation of a
//! start whose outcome is unknown, and the lifecycle of a Test. The decisions are in `trust-control`; each module here
//! makes the runtime calls in the order those decisions need, and reaches the runtime only through the traits it declares.

pub mod identify;
pub mod identify_campaign;
pub mod identify_recovery;
pub mod reconcile;
pub mod test_experiment_lifecycle;
pub mod validate;
pub mod validate_model_check;
