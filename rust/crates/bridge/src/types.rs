//! Bridge types for remote session management

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Default per-session timeout (24 hours)
pub const DEFAULT_SESSION_TIMEOUT_MS: u64 = 24 * 60 * 60 * 1000;

/// Login instruction for bridge auth errors
pub const BRIDGE_LOGIN_INSTRUCTION: &str = 
    "Remote Control is only available with claude.ai subscriptions. Please use `/login` to sign in with your claude.ai account.";

/// Full error when remote-control is run without auth
pub const BRIDGE_LOGIN_ERROR: &str = 
    "Error: You must be logged in to use Remote Control.\n\nRemote Control is only available with claude.ai subscriptions. Please use `/login` to sign in with your claude.ai account.";

/// Message shown when user disconnects Remote Control
pub const REMOTE_CONTROL_DISCONNECTED_MSG: &str = "Remote Control disconnected.";

/// Work data type
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WorkData {
    #[serde(rename = "session")]
    Session { id: String },
    #[serde(rename = "healthcheck")]
    Healthcheck { id: String },
}

/// Work response from the server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkResponse {
    pub id: String,
    #[serde(rename = "type")]
    pub work_type: String,
    pub environment_id: String,
    pub state: String,
    pub data: WorkData,
    pub secret: String, // base64url-encoded JSON
    pub created_at: String,
}

/// Work secret decoded from base64
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkSecret {
    pub version: u32,
    pub session_ingress_token: String,
    pub api_base_url: String,
    pub sources: Vec<WorkSource>,
    pub auth: Vec<WorkAuth>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claude_code_args: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mcp_config: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment_variables: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_code_sessions: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkSource {
    #[serde(rename = "type")]
    pub source_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_info: Option<GitInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitInfo {
    #[serde(rename = "type")]
    pub git_type: String,
    pub repo: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ref_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkAuth {
    #[serde(rename = "type")]
    pub auth_type: String,
    pub token: String,
}

/// Session completion status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionDoneStatus {
    Completed,
    Failed,
    Interrupted,
}

/// Session activity type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionActivityType {
    ToolStart,
    Text,
    Result,
    Error,
}

/// Session activity record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionActivity {
    #[serde(rename = "type")]
    pub activity_type: SessionActivityType,
    pub summary: String,
    pub timestamp: u64,
}

/// Spawn mode for session working directories
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SpawnMode {
    SingleSession,
    Worktree,
    SameDir,
}

/// Worker type for bridge
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BridgeWorkerType {
    ClaudeCode,
    ClaudeCodeAssistant,
}

/// Bridge configuration
#[derive(Debug, Clone)]
pub struct BridgeConfig {
    pub dir: String,
    pub machine_name: String,
    pub branch: String,
    pub git_repo_url: Option<String>,
    pub max_sessions: usize,
    pub spawn_mode: SpawnMode,
    pub verbose: bool,
    pub sandbox: bool,
    pub bridge_id: String,
    pub worker_type: String,
    pub environment_id: String,
    pub reuse_environment_id: Option<String>,
    pub api_base_url: String,
    pub session_ingress_url: String,
    pub debug_file: Option<String>,
    pub session_timeout_ms: Option<u64>,
}

/// Permission response event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionResponseEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    pub response: PermissionResponse,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionResponse {
    pub subtype: String,
    pub request_id: String,
    pub response: HashMap<String, serde_json::Value>,
}

/// Session spawn options
pub struct SessionSpawnOpts {
    pub session_id: String,
    pub sdk_url: String,
    pub access_token: String,
    pub use_ccr_v2: bool,
    pub worker_epoch: Option<u64>,
    pub on_first_user_message: Option<Box<dyn Fn(String) + Send + Sync>>,
}

impl std::fmt::Debug for SessionSpawnOpts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionSpawnOpts")
            .field("session_id", &self.session_id)
            .field("sdk_url", &self.sdk_url)
            .field("access_token", &"<redacted>")
            .field("use_ccr_v2", &self.use_ccr_v2)
            .field("worker_epoch", &self.worker_epoch)
            .field("on_first_user_message", &self.on_first_user_message.as_ref().map(|_| "<callback>"))
            .finish()
    }
}

impl Clone for SessionSpawnOpts {
    fn clone(&self) -> Self {
        Self {
            session_id: self.session_id.clone(),
            sdk_url: self.sdk_url.clone(),
            access_token: self.access_token.clone(),
            use_ccr_v2: self.use_ccr_v2,
            worker_epoch: self.worker_epoch,
            on_first_user_message: None, // callbacks can't be cloned
        }
    }
}

/// Backoff configuration
#[derive(Debug, Clone, Copy)]
pub struct BackoffConfig {
    pub conn_initial_ms: u64,
    pub conn_cap_ms: u64,
    pub conn_give_up_ms: u64,
    pub general_initial_ms: u64,
    pub general_cap_ms: u64,
    pub general_give_up_ms: u64,
    pub shutdown_grace_ms: Option<u64>,
    pub stop_work_base_delay_ms: Option<u64>,
}

impl Default for BackoffConfig {
    fn default() -> Self {
        Self {
            conn_initial_ms: 2_000,
            conn_cap_ms: 120_000,
            conn_give_up_ms: 600_000,
            general_initial_ms: 500,
            general_cap_ms: 30_000,
            general_give_up_ms: 600_000,
            shutdown_grace_ms: Some(30_000),
            stop_work_base_delay_ms: Some(1_000),
        }
    }
}

/// Bridge error types
#[derive(Debug, thiserror::Error)]
pub enum BridgeError {
    #[error("Authentication required: {0}")]
    AuthRequired(String),
    
    #[error("Session not found: {0}")]
    SessionNotFound(String),
    
    #[error("Environment error: {0}")]
    EnvironmentError(String),
    
    #[error("Token expired: {0}")]
    TokenExpired(String),
    
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    
    #[error("API error: {0}")]
    ApiError(String),
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),
    
    #[error("HTTP error: {0}")]
    HttpError(String),
    
    #[error("Timeout: {0}")]
    Timeout(String),
    
    #[error("Internal error: {0}")]
    InternalError(String),
}

/// Environment registration result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentRegistration {
    pub environment_id: String,
    pub environment_secret: String,
}

/// Heartbeat response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatResponse {
    pub lease_extended: bool,
    pub state: String,
}

/// Session info for multi-session display
#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub session_id: String,
    pub url: String,
    pub title: Option<String>,
    pub activity: Option<SessionActivity>,
    pub started_at: u64,
}
