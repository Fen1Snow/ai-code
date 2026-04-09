//! Bridge main module for managing remote sessions

use crate::api_client::BridgeApiClient;
use crate::session_runner::{SessionHandle, SessionSpawner};
use crate::types::*;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Bridge manages remote sessions
pub struct Bridge<A: BridgeApiClient, S: SessionSpawner> {
    config: BridgeConfig,
    api_client: A,
    spawner: S,
    sessions: Arc<RwLock<HashMap<String, SessionHandle>>>,
    environment_id: Arc<RwLock<Option<String>>>,
    environment_secret: Arc<RwLock<Option<String>>>,
    shutdown_token: tokio_util::sync::CancellationToken,
}

impl<A: BridgeApiClient, S: SessionSpawner> Bridge<A, S> {
    pub fn new(config: BridgeConfig, api_client: A, spawner: S) -> Self {
        Self {
            config,
            api_client,
            spawner,
            sessions: Arc::new(RwLock::new(HashMap::new())),
            environment_id: Arc::new(RwLock::new(None)),
            environment_secret: Arc::new(RwLock::new(None)),
            shutdown_token: tokio_util::sync::CancellationToken::new(),
        }
    }

    /// Register the bridge environment
    pub async fn register(&self) -> Result<String, BridgeError> {
        let registration = self.api_client.register_bridge_environment(&self.config).await?;
        
        let mut env_id = self.environment_id.write().await;
        let mut env_secret = self.environment_secret.write().await;
        
        *env_id = Some(registration.environment_id.clone());
        *env_secret = Some(registration.environment_secret);
        
        Ok(registration.environment_id)
    }

    /// Start polling for work
    pub async fn run(&self) -> Result<(), BridgeError> {
        loop {
            tokio::select! {
                _ = self.shutdown_token.cancelled() => {
                    tracing::info!("Bridge shutdown requested");
                    break;
                }
                result = self.poll_and_process() => {
                    if let Err(e) = result {
                        tracing::error!("Poll error: {}", e);
                        // Apply backoff
                        tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
                    }
                }
            }
        }
        
        Ok(())
    }

    async fn poll_and_process(&self) -> Result<(), BridgeError> {
        let env_id = {
            let id = self.environment_id.read().await;
            id.clone().ok_or_else(|| BridgeError::InternalError("Not registered".into()))?
        };
        
        let env_secret = {
            let secret = self.environment_secret.read().await;
            secret.clone().ok_or_else(|| BridgeError::InternalError("Not registered".into()))?
        };

        let work = self.api_client.poll_for_work(
            &env_id,
            &env_secret,
            Some(self.shutdown_token.clone()),
            None,
        ).await?;

        if let Some(work_response) = work {
            self.process_work(work_response).await?;
        }

        Ok(())
    }

    async fn process_work(&self, work: WorkResponse) -> Result<(), BridgeError> {
        match work.data {
            WorkData::Session { id: session_id } => {
                // Decode work secret
                let secret: WorkSecret = decode_work_secret(&work.secret)?;
                
                // Spawn session
                let opts = SessionSpawnOpts {
                    session_id: session_id.clone(),
                    sdk_url: secret.api_base_url.clone(),
                    access_token: secret.session_ingress_token.clone(),
                    use_ccr_v2: secret.use_code_sessions.unwrap_or(false),
                    worker_epoch: None,
                    on_first_user_message: None,
                };

                let handle = self.spawner.spawn(opts, &self.config.dir);
                
                // Acknowledge work
                let access_token = handle.access_token.lock().unwrap().clone();
                self.api_client.acknowledge_work(
                    &work.environment_id,
                    &work.id,
                    &access_token,
                ).await?;

                // Store session
                let mut sessions = self.sessions.write().await;
                sessions.insert(session_id.clone(), handle);

                tracing::info!("Started session: {}", session_id);
            }
            WorkData::Healthcheck { id: _ } => {
                tracing::debug!("Received healthcheck");
            }
        }

        Ok(())
    }

    /// Shutdown the bridge
    pub async fn shutdown(&self) -> Result<(), BridgeError> {
        self.shutdown_token.cancel();
        
        // Kill all active sessions
        let mut sessions = self.sessions.write().await;
        for (_, mut handle) in sessions.drain() {
            handle.kill();
        }

        // Deregister environment
        if let Some(env_id) = self.environment_id.read().await.as_ref() {
            self.api_client.deregister_environment(env_id).await?;
        }

        Ok(())
    }
}

fn decode_work_secret(secret: &str) -> Result<WorkSecret, BridgeError> {
    // Decode base64url
    let decoded = base64_url::decode(secret)
        .map_err(|e| BridgeError::InternalError(format!("Base64 decode error: {}", e)))?;
    
    // Parse JSON
    let work_secret: WorkSecret = serde_json::from_slice(&decoded)?;
    
    Ok(work_secret)
}
