//! Bridge crate for remote REPL session management
//! 
//! This crate implements the bridge protocol for remote control sessions,
//! including environment registration, session spawning, and work polling.

pub mod types;
pub mod api_client;
pub mod session_runner;
pub mod bridge_main;
pub mod jwt_utils;
pub mod work_secret;

pub use types::*;
pub use api_client::BridgeApiClient;
pub use session_runner::{SessionSpawner, SessionHandle};
pub use bridge_main::Bridge;
