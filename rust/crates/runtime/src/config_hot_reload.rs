// 配置热重载 - 配置文件监控和自动重载
// 实现配置变更自动检测和重载

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use std::fs;
use std::thread;

use crate::config::{ConfigLoader, RuntimeConfig};

/// 配置变更事件
#[derive(Debug, Clone)]
pub enum ConfigChangeEvent {
    ConfigModified(PathBuf),
    ConfigReloaded(RuntimeConfig),
    ConfigError(String),
}

/// 配置热重载管理器
pub struct ConfigHotReloadManager {
    config_paths: Vec<PathBuf>,
    current_config: Option<RuntimeConfig>,
    running: Arc<std::sync::atomic::AtomicBool>,
    event_tx: Option<crossbeam_channel::Sender<ConfigChangeEvent>>,
    event_rx: Option<crossbeam_channel::Receiver<ConfigChangeEvent>>,
}

impl ConfigHotReloadManager {
    /// 创建新的热重载管理器
    pub fn new(config_paths: Vec<PathBuf>) -> Self {
        let (tx, rx) = crossbeam_channel::unbounded();

        Self {
            config_paths,
            current_config: None,
            running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            event_tx: Some(tx),
            event_rx: Some(rx),
        }
    }

    /// 获取事件接收器
    pub fn event_receiver(&self) -> Option<crossbeam_channel::Receiver<ConfigChangeEvent>> {
        self.event_rx.clone()
    }

    /// 初始加载配置
    pub fn initial_load(&mut self) -> Result<RuntimeConfig, String> {
        // 使用第一个配置路径的父目录作为 cwd
        let cwd = self.config_paths.first()
            .and_then(|p| p.parent())
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
        
        let loader = ConfigLoader::default_for(&cwd);
        let config = loader.load()
            .map_err(|e| format!("Failed to load config: {}", e))?;

        self.current_config = Some(config.clone());
        Ok(config)
    }

    /// 启动配置监控
    pub fn start_watching(&mut self) -> Result<(), String> {
        if self.running.load(std::sync::atomic::Ordering::Relaxed) {
            return Err("Watcher is already running".to_string());
        }

        self.running.store(true, std::sync::atomic::Ordering::Relaxed);

        let config_paths = self.config_paths.clone();
        let running = self.running.clone();
        let tx = self.event_tx.clone().ok_or("Event channel not initialized")?;

        // 记录文件修改时间
        let mut previous_state: BTreeMap<PathBuf, std::time::SystemTime> = BTreeMap::new();
        for path in &config_paths {
            if path.exists() {
                if let Ok(metadata) = fs::metadata(path) {
                    if let Ok(modified) = metadata.modified() {
                        previous_state.insert(path.clone(), modified);
                    }
                }
            }
        }

        thread::spawn(move || {
            while running.load(std::sync::atomic::Ordering::Relaxed) {
                thread::sleep(Duration::from_secs(3));

                // 检查配置文件变更
                let mut config_changed = false;

                for path in &config_paths {
                    if !path.exists() {
                        continue;
                    }

                    let current_modified = match fs::metadata(path).and_then(|m| m.modified()) {
                        Ok(m) => m,
                        Err(_) => continue,
                    };

                    match previous_state.get(path) {
                        None => {
                            // 新文件
                            config_changed = true;
                            let _ = tx.send(ConfigChangeEvent::ConfigModified(path.clone()));
                        }
                        Some(prev_modified) => {
                            if current_modified != *prev_modified {
                                // 文件修改
                                config_changed = true;
                                let _ = tx.send(ConfigChangeEvent::ConfigModified(path.clone()));
                            }
                        }
                    }
                }

                // 如果配置变更，重新加载
                if config_changed {
                    let cwd = config_paths.first()
                        .and_then(|p| p.parent())
                        .map(|p| p.to_path_buf())
                        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
                    let loader = ConfigLoader::default_for(&cwd);
                    match loader.load() {
                        Ok(new_config) => {
                            let _ = tx.send(ConfigChangeEvent::ConfigReloaded(new_config.clone()));
                        }
                        Err(e) => {
                            let _ = tx.send(ConfigChangeEvent::ConfigError(format!(
                                "Failed to reload config: {}",
                                e
                            )));
                        }
                    }

                    // 更新状态
                    previous_state.clear();
                    for path in &config_paths {
                        if path.exists() {
                            if let Ok(metadata) = fs::metadata(path) {
                                if let Ok(modified) = metadata.modified() {
                                    previous_state.insert(path.clone(), modified);
                                }
                            }
                        }
                    }
                }
            }
        });

        Ok(())
    }

    /// 停止监控
    pub fn stop_watching(&mut self) {
        self.running.store(false, std::sync::atomic::Ordering::Relaxed);
    }

    /// 获取当前配置
    pub fn current_config(&self) -> Option<&RuntimeConfig> {
        self.current_config.as_ref()
    }

    /// 手动重载配置
    pub fn reload(&mut self) -> Result<RuntimeConfig, String> {
        let cwd = self.config_paths.first()
            .and_then(|p| p.parent())
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
        
        let loader = ConfigLoader::default_for(&cwd);
        let config = loader.load()
            .map_err(|e| format!("Failed to reload config: {}", e))?;

        self.current_config = Some(config.clone());

        if let Some(tx) = &self.event_tx {
            let _ = tx.send(ConfigChangeEvent::ConfigReloaded(config.clone()));
        }

        Ok(config)
    }
}

impl Drop for ConfigHotReloadManager {
    fn drop(&mut self) {
        self.stop_watching();
    }
}

/// 简化的配置监控器（无依赖版本）
pub struct ConfigWatcher {
    config_paths: Vec<PathBuf>,
    running: Arc<std::sync::atomic::AtomicBool>,
}

impl ConfigWatcher {
    pub fn new(config_paths: Vec<PathBuf>) -> Self {
        Self {
            config_paths,
            running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// 启动监控（简化版，仅打印日志）
    pub fn start<F>(&self, callback: F) -> Result<(), String>
    where
        F: Fn(ConfigChangeEvent) + Send + 'static,
    {
        if self.running.load(std::sync::atomic::Ordering::Relaxed) {
            return Err("Watcher is already running".to_string());
        }

        self.running.store(true, std::sync::atomic::Ordering::Relaxed);

        let config_paths = self.config_paths.clone();
        let running = self.running.clone();

        thread::spawn(move || {
            let mut previous_state: BTreeMap<PathBuf, std::time::SystemTime> = BTreeMap::new();

            while running.load(std::sync::atomic::Ordering::Relaxed) {
                thread::sleep(Duration::from_secs(3));

                for path in &config_paths {
                    if !path.exists() {
                        continue;
                    }

                    let current_modified = match fs::metadata(path).and_then(|m| m.modified()) {
                        Ok(m) => m,
                        Err(_) => continue,
                    };

                    match previous_state.get(path) {
                        None => {
                            callback(ConfigChangeEvent::ConfigModified(path.clone()));
                        }
                        Some(prev_modified) => {
                            if current_modified != *prev_modified {
                                callback(ConfigChangeEvent::ConfigModified(path.clone()));

                                // 重新加载配置
                                let cwd = config_paths.first()
                                    .and_then(|p| p.parent())
                                    .map(|p| p.to_path_buf())
                                    .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
                                let loader = ConfigLoader::default_for(&cwd);
                                if let Ok(new_config) = loader.load() {
                                    callback(ConfigChangeEvent::ConfigReloaded(new_config));
                                }
                            }
                        }
                    }
                }

                // 更新状态
                previous_state.clear();
                for path in &config_paths {
                    if path.exists() {
                        if let Ok(metadata) = fs::metadata(path) {
                            if let Ok(modified) = metadata.modified() {
                                previous_state.insert(path.clone(), modified);
                            }
                        }
                    }
                }
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
    use std::io::Write;

    #[test]
    fn test_manager_creation() {
        let temp_dir = env::temp_dir().join("test-config");
        fs::create_dir_all(&temp_dir).ok();

        let config_path = temp_dir.join("settings.json");
        let manager = ConfigHotReloadManager::new(vec![config_path]);

        assert!(manager.event_receiver().is_some());

        // 清理
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_watcher_start_stop() {
        let temp_dir = env::temp_dir().join("test-config-watch");
        fs::create_dir_all(&temp_dir).ok();

        let config_path = temp_dir.join("settings.json");
        let mut watcher = ConfigWatcher::new(vec![config_path]);

        let result = watcher.start(|event| {
            println!("Config event: {:?}", event);
        });

        assert!(result.is_ok());

        // 运行一小段时间
        thread::sleep(Duration::from_millis(100));

        watcher.stop();

        // 清理
        let _ = fs::remove_dir_all(&temp_dir);
    }
}
