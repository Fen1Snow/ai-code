// Skill 热加载 - 文件系统监控和自动重新加载
// 实现技能目录的文件系统监控，支持自动重新加载

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use std::fs;
use std::thread;

use serde::{Deserialize, Serialize};

/// Skill 变更事件
#[derive(Debug, Clone)]
pub enum SkillChangeEvent {
    SkillAdded(String),
    SkillRemoved(String),
    SkillModified(String),
}

/// Skill 热加载管理器
pub struct SkillHotReloadManager {
    skills_dirs: Vec<PathBuf>,
    loaded_skills: BTreeMap<String, SkillMetadata>,
    event_tx: Option<crossbeam_channel::Sender<SkillChangeEvent>>,
    event_rx: Option<crossbeam_channel::Receiver<SkillChangeEvent>>,
    running: Arc<std::sync::atomic::AtomicBool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMetadata {
    pub name: String,
    pub path: PathBuf,
    pub description: Option<String>,
    pub version: Option<String>,
    pub category: Option<String>,
    pub last_modified: std::time::SystemTime,
}

impl SkillHotReloadManager {
    /// 创建新的热加载管理器
    pub fn new(skills_dirs: Vec<PathBuf>) -> Self {
        let (tx, rx) = crossbeam_channel::unbounded();
        
        Self {
            skills_dirs,
            loaded_skills: BTreeMap::new(),
            event_tx: Some(tx),
            event_rx: Some(rx),
            running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// 获取事件接收器
    pub fn event_receiver(&self) -> Option<crossbeam_channel::Receiver<SkillChangeEvent>> {
        self.event_rx.clone()
    }

    /// 初始加载所有 Skills
    pub fn initial_load(&mut self) -> Result<Vec<SkillMetadata>, String> {
        let mut skills = Vec::new();
        
        for dir in &self.skills_dirs {
            if !dir.exists() {
                continue;
            }
            
            let dir_skills = self.load_skills_from_dir(dir)?;
            for skill in dir_skills {
                self.loaded_skills.insert(skill.name.clone(), skill.clone());
                skills.push(skill);
            }
        }
        
        Ok(skills)
    }

    /// 从目录加载 Skills
    fn load_skills_from_dir(&self, dir: &Path) -> Result<Vec<SkillMetadata>, String> {
        let mut skills = Vec::new();
        
        let entries = fs::read_dir(dir)
            .map_err(|e| format!("Failed to read directory {}: {}", dir.display(), e))?;
        
        for entry in entries {
            let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
            let path = entry.path();
            
            if !path.is_dir() {
                continue;
            }
            
            let skill_path = path.join("SKILL.md");
            if !skill_path.exists() {
                continue;
            }
            
            if let Some(metadata) = self.parse_skill_metadata(&skill_path)? {
                skills.push(metadata);
            }
        }
        
        Ok(skills)
    }

    /// 解析 Skill 元数据
    fn parse_skill_metadata(&self, path: &Path) -> Result<Option<SkillMetadata>, String> {
        let content = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read file: {}", e))?;
        
        let mut name = None;
        let mut description = None;
        let mut version = None;
        let mut category = None;
        
        let mut in_frontmatter = false;
        
        for line in content.lines() {
            if line.trim() == "---" {
                if !in_frontmatter {
                    in_frontmatter = true;
                } else {
                    break;
                }
                continue;
            }
            
            if in_frontmatter {
                if let Some(value) = line.strip_prefix("name = ") {
                    name = Some(value.trim().trim_matches('"').to_string());
                } else if let Some(value) = line.strip_prefix("description = ") {
                    description = Some(value.trim().trim_matches('"').to_string());
                } else if let Some(value) = line.strip_prefix("version = ") {
                    version = Some(value.trim().trim_matches('"').to_string());
                } else if let Some(value) = line.strip_prefix("category = ") {
                    category = Some(value.trim().trim_matches('"').to_string());
                }
            }
        }
        
        let name = match name {
            Some(n) => n,
            None => return Ok(None),
        };
        
        let last_modified = fs::metadata(path)
            .and_then(|m| m.modified())
            .unwrap_or_else(|_| std::time::SystemTime::now());
        
        Ok(Some(SkillMetadata {
            name,
            path: path.to_path_buf(),
            description,
            version,
            category,
            last_modified,
        }))
    }

    /// 启动文件监控
    pub fn start_watching(&mut self) -> Result<(), String> {
        if self.running.load(std::sync::atomic::Ordering::Relaxed) {
            return Err("Watcher is already running".to_string());
        }
        
        self.running.store(true, std::sync::atomic::Ordering::Relaxed);
        
        let skills_dirs = self.skills_dirs.clone();
        let running = self.running.clone();
        let tx = self.event_tx.clone().ok_or("Event channel not initialized")?;
        
        // 克隆 loaded_skills 用于比较
        let mut previous_state: BTreeMap<String, std::time::SystemTime> = 
            self.loaded_skills
                .iter()
                .map(|(name, meta)| (name.clone(), meta.last_modified))
                .collect();
        
        thread::spawn(move || {
            while running.load(std::sync::atomic::Ordering::Relaxed) {
                thread::sleep(Duration::from_secs(2));
                
                // 扫描所有 skills 目录
                let mut current_state: BTreeMap<String, (PathBuf, std::time::SystemTime)> = 
                    BTreeMap::new();
                
                for dir in &skills_dirs {
                    if !dir.exists() {
                        continue;
                    }
                    
                    if let Ok(entries) = fs::read_dir(dir) {
                        for entry in entries.flatten() {
                            let path = entry.path();
                            if !path.is_dir() {
                                continue;
                            }
                            
                            let skill_path = path.join("SKILL.md");
                            if !skill_path.exists() {
                                continue;
                            }
                            
                            if let Ok(metadata) = fs::metadata(&skill_path) {
                                if let Ok(modified) = metadata.modified() {
                                    if let Some(stem) = path.file_name() {
                                        let skill_name = stem.to_string_lossy().to_string();
                                        current_state.insert(skill_name, (skill_path, modified));
                                    }
                                }
                            }
                        }
                    }
                }
                
                // 检测变更
                // 1. 检测新增和修改
                for (name, (_path, modified)) in &current_state {
                    match previous_state.get(name) {
                        None => {
                            // 新增
                            let _ = tx.send(SkillChangeEvent::SkillAdded(name.clone()));
                        }
                        Some(prev_modified) => {
                            if modified != prev_modified {
                                // 修改
                                let _ = tx.send(SkillChangeEvent::SkillModified(name.clone()));
                            }
                        }
                    }
                }
                
                // 2. 检测删除
                for name in previous_state.keys() {
                    if !current_state.contains_key(name) {
                        let _ = tx.send(SkillChangeEvent::SkillRemoved(name.clone()));
                    }
                }
                
                // 更新状态
                previous_state = current_state
                    .into_iter()
                    .map(|(name, (_, modified))| (name, modified))
                    .collect();
            }
        });
        
        Ok(())
    }

    /// 停止监控
    pub fn stop_watching(&mut self) {
        self.running.store(false, std::sync::atomic::Ordering::Relaxed);
    }

    /// 重新加载指定 Skill
    pub fn reload_skill(&mut self, name: &str) -> Result<Option<SkillMetadata>, String> {
        // 在所有目录中查找
        for dir in &self.skills_dirs {
            let skill_path = dir.join(name).join("SKILL.md");
            if skill_path.exists() {
                if let Some(metadata) = self.parse_skill_metadata(&skill_path)? {
                    self.loaded_skills.insert(name.to_string(), metadata.clone());
                    
                    if let Some(tx) = &self.event_tx {
                        let _ = tx.send(SkillChangeEvent::SkillModified(name.to_string()));
                    }
                    
                    return Ok(Some(metadata));
                }
            }
        }
        
        // 如果找不到，从已加载的移除
        self.loaded_skills.remove(name);
        
        if let Some(tx) = &self.event_tx {
            let _ = tx.send(SkillChangeEvent::SkillRemoved(name.to_string()));
        }
        
        Ok(None)
    }

    /// 获取已加载的 Skills
    pub fn list_loaded_skills(&self) -> Vec<&SkillMetadata> {
        self.loaded_skills.values().collect()
    }

    /// 获取指定 Skill
    pub fn get_skill(&self, name: &str) -> Option<&SkillMetadata> {
        self.loaded_skills.get(name)
    }
}

impl Drop for SkillHotReloadManager {
    fn drop(&mut self) {
        self.stop_watching();
    }
}

/// 简化的 Skill 监控器（无依赖版本）
pub struct SkillWatcher {
    skills_dirs: Vec<PathBuf>,
    running: Arc<std::sync::atomic::AtomicBool>,
}

impl SkillWatcher {
    pub fn new(skills_dirs: Vec<PathBuf>) -> Self {
        Self {
            skills_dirs,
            running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// 启动监控（简化版，仅打印日志）
    pub fn start<F>(&self, callback: F) -> Result<(), String>
    where
        F: Fn(SkillChangeEvent) + Send + 'static,
    {
        if self.running.load(std::sync::atomic::Ordering::Relaxed) {
            return Err("Watcher is already running".to_string());
        }
        
        self.running.store(true, std::sync::atomic::Ordering::Relaxed);
        
        let skills_dirs = self.skills_dirs.clone();
        let running = self.running.clone();
        
        thread::spawn(move || {
            let mut previous_state: BTreeMap<String, std::time::SystemTime> = BTreeMap::new();
            
            while running.load(std::sync::atomic::Ordering::Relaxed) {
                thread::sleep(Duration::from_secs(2));
                
                let mut current_state: BTreeMap<String, std::time::SystemTime> = BTreeMap::new();
                
                for dir in &skills_dirs {
                    if !dir.exists() {
                        continue;
                    }
                    
                    if let Ok(entries) = fs::read_dir(dir) {
                        for entry in entries.flatten() {
                            let path = entry.path();
                            if !path.is_dir() {
                                continue;
                            }
                            
                            let skill_path = path.join("SKILL.md");
                            if !skill_path.exists() {
                                continue;
                            }
                            
                            if let Ok(metadata) = fs::metadata(&skill_path) {
                                if let Ok(modified) = metadata.modified() {
                                    if let Some(stem) = path.file_name() {
                                        let skill_name = stem.to_string_lossy().to_string();
                                        current_state.insert(skill_name, modified);
                                    }
                                }
                            }
                        }
                    }
                }
                
                // 检测变更并调用回调
                for name in current_state.keys() {
                    if !previous_state.contains_key(name) {
                        callback(SkillChangeEvent::SkillAdded(name.clone()));
                    }
                }
                
                for name in previous_state.keys() {
                    if !current_state.contains_key(name) {
                        callback(SkillChangeEvent::SkillRemoved(name.clone()));
                    }
                }
                
                previous_state = current_state;
            }
        });
        
        Ok(())
    }

    pub fn stop(&mut self) {
        self.running.store(false, std::sync::atomic::Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs;

    #[test]
    fn test_manager_creation() {
        let temp_dir = env::temp_dir().join("test-skills");
        let manager = SkillHotReloadManager::new(vec![temp_dir]);
        assert!(manager.event_receiver().is_some());
    }

    #[test]
    fn test_initial_load() {
        let temp_dir = env::temp_dir().join("test-skills-load");
        fs::create_dir_all(&temp_dir).ok();
        
        // 创建一个测试 Skill
        let skill_dir = temp_dir.join("test-skill");
        fs::create_dir_all(&skill_dir).ok();
        fs::write(skill_dir.join("SKILL.md"), r#"---
name = "test-skill"
description = "Test skill"
---
Test instructions
"#).ok();
        
        let mut manager = SkillHotReloadManager::new(vec![temp_dir.clone()]);
        let skills = manager.initial_load();
        
        assert!(skills.is_ok());
        assert_eq!(skills.unwrap().len(), 1);
        
        // 清理
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_watcher_start_stop() {
        let temp_dir = env::temp_dir().join("test-skills-watch");
        fs::create_dir_all(&temp_dir).ok();
        
        let mut watcher = SkillWatcher::new(vec![temp_dir.clone()]);
        
        let result = watcher.start(|event| {
            println!("Event: {:?}", event);
        });
        
        assert!(result.is_ok());
        
        // 运行一小段时间
        thread::sleep(Duration::from_millis(100));
        
        watcher.stop();
        
        // 清理
        let _ = fs::remove_dir_all(&temp_dir);
    }
}
