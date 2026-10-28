//! Who may operate the HMI and what it may write (specification 39, §12.2–12.4, §12.8): the
//! users, the session times and the runtime's write policy, read from `[runtime.web.hmi]`.

pub mod config;
pub mod page_policy;
pub mod password;
pub mod policy;
pub mod sessions;

#[cfg(test)]
mod page_policy_tests;
#[cfg(test)]
mod policy_tests;
#[cfg(test)]
mod sessions_tests;
#[cfg(test)]
mod tests;
