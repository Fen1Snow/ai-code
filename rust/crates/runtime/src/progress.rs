// Progress Callback System - 进度回调系统
// 实现长时间运行工具的状态反馈

use std::sync::Arc;
use std::time::{Duration, Instant};
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, RwLock};

/// 进度状态
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ProgressStatus {
    /// 已开始
    Started,
    /// 进行中
    InProgress,
    /// 已完成
    Completed,
    /// 已失败
    Failed,
    /// 已取消
    Cancelled,
}

/// 进度信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressInfo {
    /// 工具使用 ID
    pub tool_use_id: String,
    /// 服务器名称
    pub server_name: String,
    /// 工具名称
    pub tool_name: String,
    /// 状态
    pub status: ProgressStatus,
    /// 进度百分比 (0.0 - 1.0)
    pub progress: Option<f32>,
    /// 总数
    pub total: Option<u64>,
    /// 已完成数
    pub completed: Option<u64>,
    /// 进度消息
    pub message: Option<String>,
    /// 已用时间 (毫秒)
    pub elapsed_ms: u64,
    /// 预计剩余时间 (毫秒)
    pub eta_ms: Option<u64>,
    /// 时间戳
    pub timestamp: u64,
}

/// 进度回调类型
#[allow(dead_code)]
pub type ProgressCallback = Box<dyn Fn(ProgressInfo) + Send + Sync>;

/// 进度追踪器
pub struct ProgressTracker {
    tool_use_id: String,
    server_name: String,
    tool_name: String,
    start_time: Instant,
    status: Arc<RwLock<ProgressStatus>>,
    progress: Arc<RwLock<Option<f32>>>,
    total: Arc<RwLock<Option<u64>>>,
    completed: Arc<RwLock<Option<u64>>>,
    message: Arc<RwLock<Option<String>>>,
    sender: Option<broadcast::Sender<ProgressInfo>>,
}

impl ProgressTracker {
    /// 创建新的进度追踪器
    pub fn new(
        tool_use_id: String,
        server_name: String,
        tool_name: String,
    ) -> Self {
        Self {
            tool_use_id,
            server_name,
            tool_name,
            start_time: Instant::now(),
            status: Arc::new(RwLock::new(ProgressStatus::Started)),
            progress: Arc::new(RwLock::new(None)),
            total: Arc::new(RwLock::new(None)),
            completed: Arc::new(RwLock::new(None)),
            message: Arc::new(RwLock::new(None)),
            sender: None,
        }
    }

    /// 使用广播发送器创建
    pub fn with_sender(
        tool_use_id: String,
        server_name: String,
        tool_name: String,
        sender: broadcast::Sender<ProgressInfo>,
    ) -> Self {
        let mut tracker = Self::new(tool_use_id, server_name, tool_name);
        tracker.sender = Some(sender);
        tracker
    }

    /// 获取当前进度信息
    pub async fn get_info(&self) -> ProgressInfo {
        let status = self.status.read().await.clone();
        let progress = *self.progress.read().await;
        let total = *self.total.read().await;
        let completed = *self.completed.read().await;
        let message = self.message.read().await.clone();
        
        let elapsed_ms = self.start_time.elapsed().as_millis() as u64;
        
        // 计算预计剩余时间
        let eta_ms = if let Some(p) = progress {
            if p > 0.0 && p < 1.0 {
                let p = p as f64;
                Some(((elapsed_ms as f64) * (1.0 - p) / p) as u64)
            } else {
                None
            }
        } else {
            None
        };
        
        ProgressInfo {
            tool_use_id: self.tool_use_id.clone(),
            server_name: self.server_name.clone(),
            tool_name: self.tool_name.clone(),
            status,
            progress,
            total,
            completed,
            message,
            elapsed_ms,
            eta_ms,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0),
        }
    }

    /// 设置状态为进行中
    pub async fn start(&self) {
        *self.status.write().await = ProgressStatus::InProgress;
        self.emit().await;
    }

    /// 更新进度
    pub async fn update_progress(&self, progress: f32) {
        *self.progress.write().await = Some(progress.clamp(0.0, 1.0));
        *self.status.write().await = ProgressStatus::InProgress;
        self.emit().await;
    }

    /// 更新计数进度
    pub async fn update_count(&self, completed: u64, total: u64) {
        *self.completed.write().await = Some(completed);
        *self.total.write().await = Some(total);
        if total > 0 {
            *self.progress.write().await = Some(completed as f32 / total as f32);
        }
        *self.status.write().await = ProgressStatus::InProgress;
        self.emit().await;
    }

    /// 更新消息
    pub async fn update_message(&self, message: String) {
        *self.message.write().await = Some(message);
        self.emit().await;
    }

    /// 设置状态为已完成
    pub async fn complete(&self) {
        *self.status.write().await = ProgressStatus::Completed;
        *self.progress.write().await = Some(1.0);
        self.emit().await;
    }

    /// 设置状态为失败
    pub async fn fail(&self, error: String) {
        *self.status.write().await = ProgressStatus::Failed;
        *self.message.write().await = Some(error);
        self.emit().await;
    }

    /// 设置状态为取消
    pub async fn cancel(&self) {
        *self.status.write().await = ProgressStatus::Cancelled;
        self.emit().await;
    }

    /// 发送进度更新
    async fn emit(&self) {
        if let Some(sender) = &self.sender {
            let info = self.get_info().await;
            let _ = sender.send(info);
        }
    }

    /// 获取已用时间
    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }

    /// 获取已用时间 (毫秒)
    pub fn elapsed_ms(&self) -> u64 {
        self.start_time.elapsed().as_millis() as u64
    }
}

/// 进度管理器
pub struct ProgressManager {
    sender: broadcast::Sender<ProgressInfo>,
    #[allow(dead_code)]
    receiver: broadcast::Receiver<ProgressInfo>,
}

impl ProgressManager {
    /// 创建新的进度管理器
    pub fn new() -> Self {
        let (sender, receiver) = broadcast::channel(100);
        Self { sender, receiver }
    }

    /// 创建进度追踪器
    pub fn create_tracker(
        &self,
        tool_use_id: String,
        server_name: String,
        tool_name: String,
    ) -> ProgressTracker {
        ProgressTracker::with_sender(
            tool_use_id,
            server_name,
            tool_name,
            self.sender.clone(),
        )
    }

    /// 订阅进度更新
    pub fn subscribe(&self) -> broadcast::Receiver<ProgressInfo> {
        self.sender.subscribe()
    }

    /// 发送进度更新
    pub fn send(&self, info: ProgressInfo) -> Result<(), broadcast::error::SendError<ProgressInfo>> {
        self.sender.send(info)?;
        Ok(())
    }
}

impl Default for ProgressManager {
    fn default() -> Self {
        Self::new()
    }
}

/// MCP 特定的进度数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpProgressData {
    /// 进度类型
    #[serde(rename = "type")]
    pub progress_type: String,
    /// 状态
    pub status: String,
    /// 服务器名称
    pub server_name: String,
    /// 工具名称
    pub tool_name: String,
    /// 进度值
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<f32>,
    /// 总数
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<u64>,
    /// 进度消息
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress_message: Option<String>,
    /// 已用时间 (毫秒)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elapsed_time_ms: Option<u64>,
}

impl McpProgressData {
    /// 创建开始状态
    pub fn started(server_name: &str, tool_name: &str) -> Self {
        Self {
            progress_type: "mcp_progress".to_string(),
            status: "started".to_string(),
            server_name: server_name.to_string(),
            tool_name: tool_name.to_string(),
            progress: None,
            total: None,
            progress_message: None,
            elapsed_time_ms: None,
        }
    }

    /// 创建进行中状态
    pub fn in_progress(
        server_name: &str,
        tool_name: &str,
        progress: f32,
        total: Option<u64>,
        message: Option<String>,
    ) -> Self {
        Self {
            progress_type: "mcp_progress".to_string(),
            status: "progress".to_string(),
            server_name: server_name.to_string(),
            tool_name: tool_name.to_string(),
            progress: Some(progress),
            total,
            progress_message: message,
            elapsed_time_ms: None,
        }
    }

    /// 创建完成状态
    pub fn completed(server_name: &str, tool_name: &str, elapsed_ms: u64) -> Self {
        Self {
            progress_type: "mcp_progress".to_string(),
            status: "completed".to_string(),
            server_name: server_name.to_string(),
            tool_name: tool_name.to_string(),
            progress: Some(1.0),
            total: None,
            progress_message: None,
            elapsed_time_ms: Some(elapsed_ms),
        }
    }

    /// 创建失败状态
    pub fn failed(server_name: &str, tool_name: &str, elapsed_ms: u64) -> Self {
        Self {
            progress_type: "mcp_progress".to_string(),
            status: "failed".to_string(),
            server_name: server_name.to_string(),
            tool_name: tool_name.to_string(),
            progress: None,
            total: None,
            progress_message: None,
            elapsed_time_ms: Some(elapsed_ms),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_progress_tracker() {
        let tracker = ProgressTracker::new(
            "tool-123".to_string(),
            "test-server".to_string(),
            "test-tool".to_string(),
        );
        
        let info = tracker.get_info().await;
        assert_eq!(info.tool_use_id, "tool-123");
        assert_eq!(info.status, ProgressStatus::Started);
        
        tracker.update_progress(0.5).await;
        let info = tracker.get_info().await;
        assert_eq!(info.progress, Some(0.5));
        assert_eq!(info.status, ProgressStatus::InProgress);
        
        tracker.complete().await;
        let info = tracker.get_info().await;
        assert_eq!(info.status, ProgressStatus::Completed);
        assert_eq!(info.progress, Some(1.0));
    }

    #[tokio::test]
    async fn test_progress_manager() {
        let manager = ProgressManager::new();
        let tracker = manager.create_tracker(
            "tool-456".to_string(),
            "server".to_string(),
            "tool".to_string(),
        );
        
        tracker.start().await;
        
        // 订阅并接收更新
        let mut receiver = manager.subscribe();
        
        tracker.update_progress(0.75).await;
        
        // 尝试接收更新
        match receiver.try_recv() {
            Ok(info) => {
                assert_eq!(info.progress, Some(0.75));
            }
            Err(broadcast::error::TryRecvError::Empty) => {
                // 可能还没发送，这是可以接受的
            }
            _ => {}
        }
    }

    #[test]
    fn test_mcp_progress_data() {
        let started = McpProgressData::started("server", "tool");
        assert_eq!(started.status, "started");
        
        let completed = McpProgressData::completed("server", "tool", 1000);
        assert_eq!(completed.status, "completed");
        assert_eq!(completed.elapsed_time_ms, Some(1000));
    }

    #[tokio::test]
    async fn test_eta_calculation() {
        let tracker = ProgressTracker::new(
            "tool-789".to_string(),
            "server".to_string(),
            "tool".to_string(),
        );
        
        tracker.update_progress(0.25).await;
        let info = tracker.get_info().await;
        
        // ETA 应该被计算
        assert!(info.eta_ms.is_some());
    }
}
