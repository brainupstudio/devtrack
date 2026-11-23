//! DevTrack shared core: everything except the window.
//!
//! The desktop shell (`src/main.rs` + `src/ui`) is one consumer; a web or mobile shell
//! can reuse this library unchanged. Nothing in here depends on egui.

pub mod edit;
pub mod facts;
pub mod integrations;
pub mod mcp;
pub mod mcp_azure;
pub mod model;
pub mod report;
pub mod sample;
pub mod secrets;
pub mod store;
pub mod sync;
pub mod timefmt;
pub mod update;
