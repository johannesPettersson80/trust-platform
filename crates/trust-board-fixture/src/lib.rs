//! M2E fixture shared by the truST board compositions: the runner, its trace
//! records, bundle decode, and heap and stack measurement. A board composition
//! implements the interfaces in [`board`] and owns its memory map and wiring.
#![no_std]

extern crate alloc;

pub mod application;
pub mod board;
pub mod memory;
pub mod records;
pub mod runner;
pub mod services;
pub mod stack;
