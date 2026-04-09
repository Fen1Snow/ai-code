//! Bridge API client for remote session communication

use crate::types::*;
use async_trait::async_trait;
use reqwest::Client;
use serde_json::json;
use std::time::Duration;

/// Bridge API client trait for dependency injection
#[async_trait]
pub trait BridgeApiClient: Send + Sync {
    /// Register a bridge environment
    async fn register_bridge_environment(
        &self,
        config: &BridgeConfig,
    ) -> Result<EnvironmentRegistration, BridgeError>;

    /// Poll for work items
    async fn poll_for_work(
        &self,
        environment_id: &str,
        environment_secret: &str,
        signal: Option<tokio_util::sync::CancellationToken>,
        reclaim_older_than_ms: Option<u64>,
    ) -> Result<Option<WorkResponse>, BridgeError>;

    /// Acknowledge work item
    async fn acknowledge_work(
        &self,
        environment_id: &str,
        work_id: &str,
        session_token: &str,
    ) -> Result<(), BridgeError>;

    /// Stop a work item
    async fn stop_work(
        &self,
        environment_id: &str,
        work_id: &str,
        force: bool,
    ) -> Result<(), BridgeError>;

    /// Deregister the bridge environment
    async fn deregister_environment(&self, environment_id: &str) -> Result<(), BridgeError>;

    /// Send a permission response event
    async fn send_permission_response_event(
        &self,
        session_id: &str,
        event: &PermissionResponseEvent,
        session_token: &str,
    ) -> Result<(), BridgeError>;

    /// Archive a session
    async fn archive_session(&self, session_id: &str) -> Result<(), BridgeError>;

    /// Reconnect a session
    async fn reconnect_session(
        &self,
        environment_id: &str,
        session_id: &str,
    ) -> Result<(), BridgeError>;

    /// Send heartbeat for a work item
    async fn heartbeat_work(
        &self,
        environment_id: &str,
        work_id: &str,
        session_token: &str,
    ) -> Result<HeartbeatResponse, BridgeError>;
}

/// HTTP-based bridge API client implementation
pub struct HttpBridgeClient {
    client: Client,
    api_base_url: String,
}

impl HttpBridgeClient {
    pub fn new(api_base_url: &str) -> Result<Self, BridgeError> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| BridgeError::HttpError(e.to_string()))?;

        Ok(Self {
            client,
            api_base_url: api_base_url.to_string(),
        })
    }
}

#[async_trait]
impl BridgeApiClient for HttpBridgeClient {
    async fn register_bridge_environment(
        &self,
        config: &BridgeConfig,
    ) -> Result<EnvironmentRegistration, BridgeError> {
        let url = format!("{}/environments", self.api_base_url);
        let body = json!({
            "bridge_id": config.bridge_id,
            "environment_id": config.environment_id,
            "worker_type": config.worker_type,
            "machine_name": config.machine_name,
            "branch": config.branch,
            "git_repo_url": config.git_repo_url,
            "max_sessions": config.max_sessions,
            "spawn_mode": config.spawn_mode,
            "sandbox": config.sandbox,
        });

        let response = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| BridgeError::HttpError(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(BridgeError::ApiError(format!(
                "Registration failed: {} - {}",
                status, body
            )));
        }

        response
            .json()
            .await
            .map_err(|e| BridgeError::HttpError(e.to_string()))
    }

    async fn poll_for_work(
        &self,
        environment_id: &str,
        environment_secret: &str,
        _signal: Option<tokio_util::sync::CancellationToken>,
        reclaim_older_than_ms: Option<u64>,
    ) -> Result<Option<WorkResponse>, BridgeError> {
        let url = format!(
            "{}/environments/{}/work?secret={}",
            self.api_base_url, environment_id, environment_secret
        );

        let mut request = self.client.get(&url);

        if let Some(ms) = reclaim_older_than_ms {
            request = request.query(&[("reclaim_older_than_ms", ms)]);
        }

        let response = request
            .send()
            .await
            .map_err(|e| BridgeError::HttpError(e.to_string()))?;

        if response.status() == reqwest::StatusCode::NO_CONTENT {
            return Ok(None);
        }

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(BridgeError::ApiError(format!(
                "Poll failed: {} - {}",
                status, body
            )));
        }

        response
            .json()
            .await
            .map_err(|e| BridgeError::HttpError(e.to_string()))
            .map(Some)
    }

    async fn acknowledge_work(
        &self,
        environment_id: &str,
        work_id: &str,
        session_token: &str,
    ) -> Result<(), BridgeError> {
        let url = format!(
            "{}/environments/{}/work/{}/acknowledge",
            self.api_base_url, environment_id, work_id
        );

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", session_token))
            .send()
            .await
            .map_err(|e| BridgeError::HttpError(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(BridgeError::ApiError(format!(
                "Acknowledge failed: {} - {}",
                status, body
            )));
        }

        Ok(())
    }

    async fn stop_work(
        &self,
        environment_id: &str,
        work_id: &str,
        force: bool,
    ) -> Result<(), BridgeError> {
        let url = format!(
            "{}/environments/{}/work/{}/stop",
            self.api_base_url, environment_id, work_id
        );

        let response = self
            .client
            .post(&url)
            .query(&[("force", force)])
            .send()
            .await
            .map_err(|e| BridgeError::HttpError(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(BridgeError::ApiError(format!(
                "Stop work failed: {} - {}",
                status, body
            )));
        }

        Ok(())
    }

    async fn deregister_environment(&self, environment_id: &str) -> Result<(), BridgeError> {
        let url = format!("{}/environments/{}", self.api_base_url, environment_id);

        let response = self
            .client
            .delete(&url)
            .send()
            .await
            .map_err(|e| BridgeError::HttpError(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(BridgeError::ApiError(format!(
                "Deregister failed: {} - {}",
                status, body
            )));
        }

        Ok(())
    }

    async fn send_permission_response_event(
        &self,
        session_id: &str,
        event: &PermissionResponseEvent,
        session_token: &str,
    ) -> Result<(), BridgeError> {
        let url = format!("{}/sessions/{}/events", self.api_base_url, session_id);

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", session_token))
            .json(event)
            .send()
            .await
            .map_err(|e| BridgeError::HttpError(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(BridgeError::ApiError(format!(
                "Send event failed: {} - {}",
                status, body
            )));
        }

        Ok(())
    }

    async fn archive_session(&self, session_id: &str) -> Result<(), BridgeError> {
        let url = format!("{}/sessions/{}/archive", self.api_base_url, session_id);

        let response = self
            .client
            .post(&url)
            .send()
            .await
            .map_err(|e| BridgeError::HttpError(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(BridgeError::ApiError(format!(
                "Archive session failed: {} - {}",
                status, body
            )));
        }

        Ok(())
    }

    async fn reconnect_session(
        &self,
        environment_id: &str,
        session_id: &str,
    ) -> Result<(), BridgeError> {
        let url = format!(
            "{}/environments/{}/sessions/{}/reconnect",
            self.api_base_url, environment_id, session_id
        );

        let response = self
            .client
            .post(&url)
            .send()
            .await
            .map_err(|e| BridgeError::HttpError(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(BridgeError::ApiError(format!(
                "Reconnect session failed: {} - {}",
                status, body
            )));
        }

        Ok(())
    }

    async fn heartbeat_work(
        &self,
        environment_id: &str,
        work_id: &str,
        session_token: &str,
    ) -> Result<HeartbeatResponse, BridgeError> {
        let url = format!(
            "{}/environments/{}/work/{}/heartbeat",
            self.api_base_url, environment_id, work_id
        );

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", session_token))
            .send()
            .await
            .map_err(|e| BridgeError::HttpError(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(BridgeError::ApiError(format!(
                "Heartbeat failed: {} - {}",
                status, body
            )));
        }

        response
            .json()
            .await
            .map_err(|e| BridgeError::HttpError(e.to_string()))
    }
}
