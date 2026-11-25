//! M1-D converter fixture: the Symfony app in `oracle/apps/converter-fixture`
//! as Rust, against converter contract 0.4 (docs/converter-contract.md), on
//! the ported Symfony kernel served by `symfony-runtime`.
//!
//! **Hand-authored, not converter output.** It is written in the shape
//! symfony2rust is to emit (one module per PHP class under `app`, the
//! compiled container, the compiled routes, the kernel wiring), to prove the
//! runtime interface only. The conversion gate stays open until symfony2rust
//! emits this crate and it builds and passes without manual edits (PLAN.md,
//! M1-D). `runtime` holds what the workspace crates are still missing; it is
//! not part of the generated shape.
//!
//! Upstream: none

#![forbid(unsafe_code)]

pub mod app {
    pub mod error_subscriber;
    pub mod greet_controller;
    pub mod greeter;
    pub mod greeting;
    pub mod greeting_exception;
    pub mod visit_counter;
}
pub mod container;
pub mod kernel;
pub mod routes;
pub mod runtime;

pub use kernel::{router, router_at};
