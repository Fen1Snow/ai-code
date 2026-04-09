// 会话持久化 - 会话保存和恢复
// 实现会话的保存、恢复和历史查询

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Deserialize, Serialize};

use crate::session::Session;

/// 会话元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMetadata {
    pub id: String,
    pub created_at: u64,
    pub updated_at: u64,
    pub message_count: usize,
    pub token_count: usize,
    pub title: Option<String>,
    pub tags: Vec<String>,
}

/// 会话持久化管理器
pub struct SessionPersistenceManager {
    storage_dir: PathBuf,
    sessions: BTreeMap<String, SessionMetadata>,
}

impl SessionPersistenceManager {
    /// 创建新的持久化管理器
    pub fn new(storage_dir: PathBuf) -> Result<Self, String> {
        fs::create_dir_all(&storage_dir)
            .map_err(|e| format!("Failed to create storage directory: {}", e))?;

        let mut manager = Self {
            storage_dir,
            sessions: BTreeMap::new(),
        };

        // 加载现有会话元数据
        manager.load_metadata_index()?;

        Ok(manager)
    }

    /// 加载会话元数据索引
    fn load_metadata_index(&mut self) -> Result<(), String> {
        let index_path = self.storage_dir.join("sessions_index.json");

        if !index_path.exists() {
            return Ok(());
        }

        let content = fs::read_to_string(&index_path)
            .map_err(|e| format!("Failed to read index: {}", e))?;

        let sessions: BTreeMap<String, SessionMetadata> = serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse index: {}", e))?;

        self.sessions = sessions;

        Ok(())
    }

    /// 保存会话元数据索引
    fn save_metadata_index(&self) -> Result<(), String> {
        let index_path = self.storage_dir.join("sessions_index.json");

        let content = serde_json::to_string_pretty(&self.sessions)
            .map_err(|e| format!("Failed to serialize index: {}", e))?;

        fs::write(&index_path, &content)
            .map_err(|e| format!("Failed to write index: {}", e))?;

        Ok(())
    }

    /// 保存会话
    pub fn save_session(&mut self, session: &Session) -> Result<PathBuf, String> {
        let session_path = self.storage_dir.join(format!("{}.json", session.id));

        // 序列化会话
        let content = serde_json::to_string_pretty(session)
            .map_err(|e| format!("Failed to serialize session: {}", e))?;

        // 写入文件
        fs::write(&session_path, &content)
            .map_err(|e| format!("Failed to write session: {}", e))?;

        // 更新元数据
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let metadata = SessionMetadata {
            id: session.id.clone(),
            created_at: now,
            updated_at: now,
            message_count: session.messages.len(),
            token_count: session.token_count.unwrap_or(0),
            title: session.title.clone(),
            tags: session.tags.clone().unwrap_or_default(),
        };

        self.sessions.insert(session.id.clone(), metadata);
        self.save_metadata_index()?;

        Ok(session_path)
    }

    /// 加载会话
    pub fn load_session(&self, session_id: &str) -> Result<Option<Session>, String> {
        let session_path = self.storage_dir.join(format!("{}.json", session_id));

        if !session_path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&session_path)
            .map_err(|e| format!("Failed to read session: {}", e))?;

        let session: Session = serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse session: {}", e))?;

        Ok(Some(session))
    }

    /// 删除会话
    pub fn delete_session(&mut self, session_id: &str) -> Result<(), String> {
        let session_path = self.storage_dir.join(format!("{}.json", session_id));

        if session_path.exists() {
            fs::remove_file(&session_path)
                .map_err(|e| format!("Failed to delete session: {}", e))?;
        }

        self.sessions.remove(session_id);
        self.save_metadata_index()?;

        Ok(())
    }

    /// 列出所有会话
    pub fn list_sessions(&self) -> Vec<&SessionMetadata> {
        self.sessions.values().collect()
    }

    /// 按时间排序列出会话
    pub fn list_sessions_sorted(&self, desc: bool) -> Vec<&SessionMetadata> {
        let mut sessions: Vec<&SessionMetadata> = self.sessions.values().collect();

        if desc {
            sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        } else {
            sessions.sort_by(|a, b| a.updated_at.cmp(&b.updated_at));
        }

        sessions
    }

    /// 搜索会话
    pub fn search_sessions(&self, query: &str) -> Vec<&SessionMetadata> {
        let query_lower = query.to_lowercase();

        self.sessions
            .values()
            .filter(|meta| {
                meta.title
                    .as_ref()
                    .map(|t| t.to_lowercase().contains(&query_lower))
                    .unwrap_or(false)
                    || meta.tags.iter().any(|t| t.to_lowercase().contains(&query_lower))
            })
            .collect()
    }

    /// 按标签过滤会话
    pub fn filter_by_tag(&self, tag: &str) -> Vec<&SessionMetadata> {
        self.sessions
            .values()
            .filter(|meta| meta.tags.iter().any(|t| t == tag))
            .collect()
    }

    /// 获取会话统计
    pub fn get_stats(&self) -> SessionStats {
        let total_sessions = self.sessions.len();
        let total_messages: usize = self.sessions.values().map(|m| m.message_count).sum();
        let total_tokens: usize = self.sessions.values().map(|m| m.token_count).sum();

        SessionStats {
            total_sessions,
            total_messages,
            total_tokens,
        }
    }

    /// 清理旧会话
    pub fn cleanup_old_sessions(&mut self, max_age_days: u64) -> Result<usize, String> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let max_age_secs = max_age_days * 24 * 60 * 60;
        let mut deleted_count = 0;

        let to_delete: Vec<String> = self
            .sessions
            .iter()
            .filter(|(_, meta)| now - meta.updated_at > max_age_secs)
            .map(|(id, _)| id.clone())
            .collect();

        for session_id in to_delete {
            self.delete_session(&session_id)?;
            deleted_count += 1;
        }

        Ok(deleted_count)
    }

    /// 导出会话
    pub fn export_session(&self, session_id: &str) -> Result<Option<String>, String> {
        let session = self.load_session(session_id)?;

        match session {
            Some(s) => {
                let content = serde_json::to_string_pretty(&s)
                    .map_err(|e| format!("Failed to serialize session: {}", e))?;
                Ok(Some(content))
            }
            None => Ok(None),
        }
    }

    /// 导入会话
    pub fn import_session(&mut self, content: &str) -> Result<String, String> {
        let session: Session = serde_json::from_str(content)
            .map_err(|e| format!("Failed to parse session: {}", e))?;

        let session_id = session.id.clone();
        self.save_session(&session)?;

        Ok(session_id)
    }
}

/// 会话统计
#[derive(Debug, Clone)]
pub struct SessionStats {
    pub total_sessions: usize,
    pub total_messages: usize,
    pub total_tokens: usize,
}

/// 会话存档
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionArchive {
    pub sessions: Vec<Session>,
    pub exported_at: u64,
    pub version: String,
}

impl SessionArchive {
    /// 创建新存档
    pub fn new(sessions: Vec<Session>) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Self {
            sessions,
            exported_at: now,
            version: "1.0.0".to_string(),
        }
    }

    /// 导出到文件
    pub fn export_to_file(&self, path: &Path) -> Result<(), String> {
        let content = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize archive: {}", e))?;

        fs::write(path, &content)
            .map_err(|e| format!("Failed to write archive: {}", e))?;

        Ok(())
    }

    /// 从文件导入
    pub fn import_from_file(path: &Path) -> Result<Self, String> {
        let content = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read archive: {}", e))?;

        let archive: SessionArchive = serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse archive: {}", e))?;

        Ok(archive)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs;

    #[test]
    fn test_manager_creation() {
        let temp_dir = env::temp_dir().join("test-sessions");
        fs::create_dir_all(&temp_dir).ok();

        let manager = SessionPersistenceManager::new(temp_dir.clone());
        assert!(manager.is_ok());

        // 清理
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_save_and_load_session() {
        let temp_dir = env::temp_dir().join("test-sessions-save");
        fs::create_dir_all(&temp_dir).ok();

        let mut manager = SessionPersistenceManager::new(temp_dir.clone()).unwrap();

        // 创建测试会话
        let session = Session {
            id: "test-session-1".to_string(),
            messages: Vec::new(),
            token_count: Some(100),
            title: Some("Test Session".to_string()),
            tags: Some(vec!["test".to_string()]),
            ..Default::default()
        };

        // 保存
        let result = manager.save_session(&session);
        assert!(result.is_ok());

        // 加载
        let loaded = manager.load_session("test-session-1");
        assert!(loaded.is_ok());
        assert!(loaded.unwrap().is_some());

        // 清理
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_list_sessions() {
        let temp_dir = env::temp_dir().join("test-sessions-list");
        fs::create_dir_all(&temp_dir).ok();

        let mut manager = SessionPersistenceManager::new(temp_dir.clone()).unwrap();

        // 创建多个测试会话
        for i in 0..3 {
            let session = Session {
                id: format!("test-session-{}", i),
                messages: Vec::new(),
                token_count: Some(100),
                title: Some(format!("Session {}", i)),
                tags: Some(vec!["test".to_string()]),
                ..Default::default()
            };
            manager.save_session(&session).unwrap();
        }

        // 列出
        let sessions = manager.list_sessions();
        assert_eq!(sessions.len(), 3);

        // 清理
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_search_sessions() {
        let temp_dir = env::temp_dir().join("test-sessions-search");
        fs::create_dir_all(&temp_dir).ok();

        let mut manager = SessionPersistenceManager::new(temp_dir.clone()).unwrap();

        // 创建测试会话
        let session1 = Session {
            id: "test-session-1".to_string(),
            messages: Vec::new(),
            token_count: Some(100),
            title: Some("Rust Programming".to_string()),
            tags: Some(vec!["rust".to_string(), "coding".to_string()]),
            ..Default::default()
        };

        let session2 = Session {
            id: "test-session-2".to_string(),
            messages: Vec::new(),
            token_count: Some(200),
            title: Some("Python Tips".to_string()),
            tags: Some(vec!["python".to_string(), "coding".to_string()]),
            ..Default::default()
        };

        manager.save_session(&session1).unwrap();
        manager.save_session(&session2).unwrap();

        // 搜索
        let results = manager.search_sessions("rust");
        assert_eq!(results.len(), 1);

        let results = manager.search_sessions("coding");
        assert_eq!(results.len(), 2);

        // 清理
        let _ = fs::remove_dir_all(&temp_dir);
    }
}
