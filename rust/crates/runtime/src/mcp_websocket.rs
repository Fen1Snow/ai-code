// MCP WebSocket Transport - WebSocket 传输实现
// 实现用于远程 MCP 服务器的 WebSocket 传输

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use tokio::sync::RwLock;
use tokio::time::timeout;

/// WebSocket 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketConfig {
    pub url: String,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
    #[serde(default)]
    pub tls_config: Option<TlsConfig>,
    #[serde(default = "default_reconnect")]
    pub reconnect: bool,
    #[serde(default = "default_max_reconnect_attempts")]
    pub max_reconnect_attempts: u32,
}

fn default_timeout() -> u64 { 30000 }
fn default_reconnect() -> bool { true }
fn default_max_reconnect_attempts() -> u32 { 3 }

/// TLS 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsConfig {
    #[serde(default)]
    pub ca_cert_path: Option<String>,
    #[serde(default)]
    pub client_cert_path: Option<String>,
    #[serde(default)]
    pub client_key_path: Option<String>,
    #[serde(default)]
    pub verify: bool,
}

/// WebSocket 连接状态
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
    Failed(String),
}

/// MCP WebSocket 消息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum McpWsMessage {
    #[serde(rename = "request")]
    Request {
        id: i64,
        method: String,
        params: Option<JsonValue>,
    },
    #[serde(rename = "response")]
    Response {
        id: i64,
        result: Option<JsonValue>,
        error: Option<McpError>,
    },
    #[serde(rename = "notification")]
    Notification {
        method: String,
        params: Option<JsonValue>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<JsonValue>,
}

/// WebSocket 传输客户端
pub struct WebSocketTransport {
    config: WebSocketConfig,
    state: Arc<RwLock<ConnectionState>>,
    #[allow(dead_code)]
    request_id: Arc<RwLock<i64>>,
    pending_requests: Arc<RwLock<BTreeMap<i64, tokio::sync::oneshot::Sender<Result<JsonValue, String>>>>>,
}

impl WebSocketTransport {
    /// 创建新的 WebSocket 传输
    pub fn new(config: WebSocketConfig) -> Self {
        Self {
            config,
            state: Arc::new(RwLock::new(ConnectionState::Disconnected)),
            request_id: Arc::new(RwLock::new(0)),
            pending_requests: Arc::new(RwLock::new(BTreeMap::new())),
        }
    }

    /// 获取当前连接状态
    pub async fn state(&self) -> ConnectionState {
        self.state.read().await.clone()
    }

    /// 连接到 WebSocket 服务器
    pub async fn connect(&self) -> Result<(), String> {
        let mut state = self.state.write().await;
        if *state == ConnectionState::Connected {
            return Ok(());
        }
        
        *state = ConnectionState::Connecting;
        
        // 实际的 WebSocket 连接逻辑
        // 在生产环境中，这里会使用 tokio-tungstenite 或其他 WebSocket 库
        // 这里提供框架代码
        
        let url = &self.config.url;
        
        // 验证 URL 格式
        if !url.starts_with("ws://") && !url.starts_with("wss://") {
            *state = ConnectionState::Failed("Invalid WebSocket URL".to_string());
            return Err("URL must start with ws:// or wss://".to_string());
        }
        
        // 模拟连接成功
        // 实际实现需要:
        // 1. 建立 WebSocket 连接
        // 2. 设置消息处理器
        // 3. 处理重连逻辑
        
        *state = ConnectionState::Connected;
        
        Ok(())
    }

    /// 断开连接
    pub async fn disconnect(&self) -> Result<(), String> {
        let mut state = self.state.write().await;
        *state = ConnectionState::Disconnected;
        
        // 清理所有待处理的请求
        let mut pending = self.pending_requests.write().await;
        let keys: Vec<_> = pending.keys().copied().collect();
        for key in keys {
            if let Some(sender) = pending.remove(&key) {
                let _ = sender.send(Err("Connection closed".to_string()));
            }
        }
        
        Ok(())
    }

    /// 发送请求并等待响应
    pub async fn send_request(&self, method: &str, params: Option<JsonValue>) -> Result<JsonValue, String> {
        let state = self.state.read().await.clone();
        if state != ConnectionState::Connected {
            return Err("Not connected".to_string());
        }
        
        // 生成请求 ID
        let id = {
            let mut rid = self.request_id.write().await;
            *rid += 1;
            *rid
        };
        
        // 创建响应通道
        let (tx, rx) = tokio::sync::oneshot::channel();
        {
            let mut pending = self.pending_requests.write().await;
            pending.insert(id, tx);
        }
        
        // 构建请求消息
        let message = McpWsMessage::Request {
            id,
            method: method.to_string(),
            params,
        };
        
        // 发送消息 (模拟)
        let _ = serde_json::to_string(&message)
            .map_err(|e| format!("Failed to serialize message: {}", e))?;
        
        // 实际实现中，这里会通过 WebSocket 发送消息
        
        // 等待响应
        let timeout_duration = Duration::from_millis(self.config.timeout_ms);
        
        match timeout(timeout_duration, rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err("Channel closed".to_string()),
            Err(_) => {
                // 超时，移除待处理请求
                let mut pending = self.pending_requests.write().await;
                pending.remove(&id);
                Err(format!("Request timeout after {}ms", self.config.timeout_ms))
            }
        }
    }

    /// 发送通知
    pub async fn send_notification(&self, method: &str, params: Option<JsonValue>) -> Result<(), String> {
        let state = self.state.read().await.clone();
        if state != ConnectionState::Connected {
            return Err("Not connected".to_string());
        }
        
        let message = McpWsMessage::Notification {
            method: method.to_string(),
            params,
        };
        
        // 发送消息 (模拟)
        let _ = serde_json::to_string(&message)
            .map_err(|e| format!("Failed to serialize message: {}", e))?;
        
        Ok(())
    }

    /// 处理接收到的消息
    pub async fn handle_message(&self, data: &str) -> Result<(), String> {
        let message: McpWsMessage = serde_json::from_str(data)
            .map_err(|e| format!("Failed to parse message: {}", e))?;
        
        match message {
            McpWsMessage::Response { id, result, error } => {
                let mut pending = self.pending_requests.write().await;
                if let Some(sender) = pending.remove(&id) {
                    if let Some(err) = error {
                        let _ = sender.send(Err(err.message));
                    } else if let Some(res) = result {
                        let _ = sender.send(Ok(res));
                    } else {
                        let _ = sender.send(Err("No result or error in response".to_string()));
                    }
                }
            }
            McpWsMessage::Notification { method, params } => {
                // 处理通知 - 可以通过回调或通道传递给上层
                log::debug!("Received notification: {} {:?}", method, params);
            }
            McpWsMessage::Request { .. } => {
                // 作为客户端，我们通常不处理请求
                log::warn!("Received unexpected request from server");
            }
        }
        
        Ok(())
    }
}

impl Drop for WebSocketTransport {
    fn drop(&mut self) {
        // 清理资源
    }
}

/// MCP WebSocket 客户端管理器
pub struct McpWebSocketManager {
    connections: Arc<RwLock<BTreeMap<String, WebSocketTransport>>>,
}

impl McpWebSocketManager {
    pub fn new() -> Self {
        Self {
            connections: Arc::new(RwLock::new(BTreeMap::new())),
        }
    }

    /// 添加连接
    pub async fn add_connection(&self, name: String, config: WebSocketConfig) -> Result<(), String> {
        let transport = WebSocketTransport::new(config);
        transport.connect().await?;
        
        let mut connections = self.connections.write().await;
        connections.insert(name, transport);
        
        Ok(())
    }

    /// 移除连接
    pub async fn remove_connection(&self, name: &str) -> Result<(), String> {
        let mut connections = self.connections.write().await;
        if let Some(transport) = connections.remove(name) {
            transport.disconnect().await?;
        }
        Ok(())
    }

    /// 获取连接
    pub async fn get_connection(&self, name: &str) -> Option<WebSocketTransport> {
        let connections = self.connections.read().await;
        connections.get(name).cloned()
    }

    /// 向指定连接发送请求
    pub async fn send_request(
        &self,
        name: &str,
        method: &str,
        params: Option<JsonValue>,
    ) -> Result<JsonValue, String> {
        let connections = self.connections.read().await;
        let transport = connections.get(name)
            .ok_or_else(|| format!("Connection '{}' not found", name))?;
        transport.send_request(method, params).await
    }
}

impl Default for McpWebSocketManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for WebSocketTransport {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            state: self.state.clone(),
            request_id: self.request_id.clone(),
            pending_requests: self.pending_requests.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_websocket_config() {
        let config = WebSocketConfig {
            url: "wss://example.com/mcp".to_string(),
            headers: BTreeMap::new(),
            timeout_ms: 30000,
            tls_config: None,
            reconnect: true,
            max_reconnect_attempts: 3,
        };
        
        assert_eq!(config.url, "wss://example.com/mcp");
    }

    #[tokio::test]
    async fn test_transport_creation() {
        let config = WebSocketConfig {
            url: "ws://localhost:8080".to_string(),
            headers: BTreeMap::new(),
            timeout_ms: 30000,
            tls_config: None,
            reconnect: false,
            max_reconnect_attempts: 1,
        };
        
        let transport = WebSocketTransport::new(config);
        let state = transport.state().await;
        assert_eq!(state, ConnectionState::Disconnected);
    }

    #[tokio::test]
    async fn test_invalid_url() {
        let config = WebSocketConfig {
            url: "http://invalid.com".to_string(),
            headers: BTreeMap::new(),
            timeout_ms: 30000,
            tls_config: None,
            reconnect: false,
            max_reconnect_attempts: 1,
        };
        
        let transport = WebSocketTransport::new(config);
        let result = transport.connect().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_manager() {
        let manager = McpWebSocketManager::new();
        
        let config = WebSocketConfig {
            url: "ws://localhost:8080".to_string(),
            headers: BTreeMap::new(),
            timeout_ms: 30000,
            tls_config: None,
            reconnect: false,
            max_reconnect_attempts: 1,
        };
        
        // 注意：实际连接需要真实的 WebSocket 服务器
        // 这里只测试管理器创建
        assert!(manager.get_connection("test").await.is_none());
    }
}
