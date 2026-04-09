//! Session runner for spawning and managing REPL sessions

use crate::types::*;
use std::collections::VecDeque;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::oneshot;

/// Session handle for managing a running session
pub struct SessionHandle {
    pub session_id: String,
    pub access_token: Arc<Mutex<String>>,
    pub activities: Arc<Mutex<VecDeque<SessionActivity>>>,
    pub current_activity: Arc<Mutex<Option<SessionActivity>>>,
    pub last_stderr: Arc<Mutex<VecDeque<String>>>,
    done_receiver: Option<oneshot::Receiver<SessionDoneStatus>>,
    kill_sender: Option<oneshot::Sender<()>>,
    force_kill_sender: Option<oneshot::Sender<()>>,
    child: Arc<Mutex<Option<Child>>>,
}

impl SessionHandle {
    /// Wait for the session to complete
    pub async fn done(&mut self) -> SessionDoneStatus {
        if let Some(receiver) = self.done_receiver.take() {
            receiver.await.unwrap_or(SessionDoneStatus::Failed)
        } else {
            SessionDoneStatus::Failed
        }
    }

    /// Gracefully kill the session
    pub fn kill(&mut self) {
        if let Some(sender) = self.kill_sender.take() {
            let _ = sender.send(());
        }
    }

    /// Force kill the session
    pub fn force_kill(&mut self) {
        if let Some(sender) = self.force_kill_sender.take() {
            let _ = sender.send(());
        }
    }

    /// Write to stdin
    pub fn write_stdin(&self, data: &str) {
        if let Ok(mut child) = self.child.lock() {
            if let Some(ref mut process) = *child {
                use std::io::Write;
                let _ = process.stdin.as_mut().map(|stdin| stdin.write_all(data.as_bytes()));
            }
        }
    }

    /// Update the access token
    pub fn update_access_token(&self, token: String) {
        if let Ok(mut access_token) = self.access_token.lock() {
            *access_token = token;
        }
    }

    /// Add an activity to the ring buffer
    pub fn add_activity(&self, activity: SessionActivity) {
        if let Ok(mut activities) = self.activities.lock() {
            if activities.len() >= 10 {
                activities.pop_front();
            }
            activities.push_back(activity.clone());
        }
        if let Ok(mut current) = self.current_activity.lock() {
            *current = Some(activity);
        }
    }

    /// Add stderr line to the ring buffer
    pub fn add_stderr(&self, line: String) {
        if let Ok(mut stderr) = self.last_stderr.lock() {
            if stderr.len() >= 10 {
                stderr.pop_front();
            }
            stderr.push_back(line);
        }
    }
}

/// Session spawner trait for dependency injection
pub trait SessionSpawner: Send + Sync {
    fn spawn(&self, opts: SessionSpawnOpts, dir: &str) -> SessionHandle;
}

/// Default session spawner implementation
pub struct DefaultSessionSpawner {
    session_timeout_ms: Option<u64>,
}

impl DefaultSessionSpawner {
    pub fn new(session_timeout_ms: Option<u64>) -> Self {
        Self { session_timeout_ms }
    }
}

impl SessionSpawner for DefaultSessionSpawner {
    fn spawn(&self, opts: SessionSpawnOpts, dir: &str) -> SessionHandle {
        let (done_tx, done_rx) = oneshot::channel();
        let (kill_tx, mut kill_rx) = oneshot::channel();
        let (force_kill_tx, mut force_kill_rx) = oneshot::channel();

        let session_id = opts.session_id.clone();
        let access_token = Arc::new(Mutex::new(opts.access_token.clone()));
        let activities = Arc::new(Mutex::new(VecDeque::new()));
        let current_activity = Arc::new(Mutex::new(None));
        let last_stderr = Arc::new(Mutex::new(VecDeque::new()));
        let session_timeout = self.session_timeout_ms;

        // Spawn the child process
        let child = Command::new("claude")
            .arg("--remote-session")
            .arg(&session_id)
            .arg("--sdk-url")
            .arg(&opts.sdk_url)
            .current_dir(dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .ok();

        let child = Arc::new(Mutex::new(child));

        let handle = SessionHandle {
            session_id: session_id.clone(),
            access_token: access_token.clone(),
            activities: activities.clone(),
            current_activity: current_activity.clone(),
            last_stderr: last_stderr.clone(),
            done_receiver: Some(done_rx),
            kill_sender: Some(kill_tx),
            force_kill_sender: Some(force_kill_tx),
            child: child.clone(),
        };

        // Spawn a task to monitor the process
        let child_clone = child.clone();
        let _session_id_clone = session_id.clone();
        tokio::spawn(async move {
            let started = Instant::now();
            let timeout = session_timeout.map(Duration::from_millis);

            loop {
                tokio::select! {
                    _ = &mut kill_rx => {
                        if let Ok(mut c) = child_clone.lock() {
                            if let Some(ref mut process) = *c {
                                let _ = process.kill();
                            }
                        }
                        let _ = done_tx.send(SessionDoneStatus::Interrupted);
                        break;
                    }
                    _ = &mut force_kill_rx => {
                        if let Ok(mut c) = child_clone.lock() {
                            if let Some(ref mut process) = *c {
                                let _ = process.kill();
                            }
                        }
                        let _ = done_tx.send(SessionDoneStatus::Interrupted);
                        break;
                    }
                    _ = tokio::time::sleep(Duration::from_millis(100)) => {
                        // Check if process has exited
                        if let Ok(mut c) = child_clone.lock() {
                            if let Some(ref mut process) = *c {
                                match process.try_wait() {
                                    Ok(Some(status)) => {
                                        let done_status = if status.success() {
                                            SessionDoneStatus::Completed
                                        } else {
                                            SessionDoneStatus::Failed
                                        };
                                        let _ = done_tx.send(done_status);
                                        break;
                                    }
                                    Ok(None) => {
                                        // Still running
                                    }
                                    Err(_) => {
                                        let _ = done_tx.send(SessionDoneStatus::Failed);
                                        break;
                                    }
                                }
                            } else {
                                let _ = done_tx.send(SessionDoneStatus::Failed);
                                break;
                            }
                        }

                        // Check timeout
                        if let Some(t) = timeout {
                            if started.elapsed() >= t {
                                if let Ok(mut c) = child_clone.lock() {
                                    if let Some(ref mut process) = *c {
                                        let _ = process.kill();
                                    }
                                }
                                let _ = done_tx.send(SessionDoneStatus::Interrupted);
                                break;
                            }
                        }
                    }
                }
            }
        });

        handle
    }
}
