// Large Output Handler - 大型工具输出处理
// 实现工具结果持久化和截断逻辑

use std::path::{Path, PathBuf};
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

/// 默认最大输出大小 (字符数)
pub const DEFAULT_MAX_OUTPUT_SIZE: usize = 100_000;

/// 默认截断阈值 (tokens)
pub const DEFAULT_TRUNCATION_THRESHOLD: usize = 50_000;

/// 输出格式
#[derive(Debug, Clone, PartialEq)]
pub enum OutputFormat {
    Json,
    Text,
    ToolResult,
    StructuredContent,
    ContentArray,
}

/// 持久化结果
#[derive(Debug, Clone)]
pub enum PersistResult {
    Success {
        filepath: PathBuf,
        original_size: usize,
        persisted_size: usize,
    },
    Error(String),
}

/// 大输出处理器配置
#[derive(Debug, Clone)]
pub struct LargeOutputConfig {
    /// 最大输出大小 (字符数)
    pub max_output_size: usize,
    /// 截断阈值 (tokens)
    pub truncation_threshold: usize,
    /// 输出目录
    pub output_dir: PathBuf,
    /// 是否启用大文件持久化
    pub enable_persistence: bool,
    /// 最大文件保留数量
    pub max_files: usize,
}

impl Default for LargeOutputConfig {
    fn default() -> Self {
        Self {
            max_output_size: DEFAULT_MAX_OUTPUT_SIZE,
            truncation_threshold: DEFAULT_TRUNCATION_THRESHOLD,
            output_dir: std::env::temp_dir().join("claw-large-output"),
            enable_persistence: true,
            max_files: 100,
        }
    }
}

/// 大输出处理器
pub struct LargeOutputHandler {
    config: LargeOutputConfig,
}

impl LargeOutputHandler {
    /// 创建新的大输出处理器
    pub fn new(config: LargeOutputConfig) -> Self {
        // 确保输出目录存在
        if config.enable_persistence {
            let _ = fs::create_dir_all(&config.output_dir);
        }
        Self { config }
    }

    /// 使用默认配置创建
    pub fn with_defaults() -> Self {
        Self::new(LargeOutputConfig::default())
    }

    /// 检查内容是否需要处理 (过大)
    pub fn needs_handling(&self, content: &str) -> bool {
        content.len() > self.config.max_output_size
    }

    /// 估算 token 数量 (粗略估计: 4 字符 = 1 token)
    pub fn estimate_tokens(content: &str) -> usize {
        content.len() / 4
    }

    /// 处理大输出
    pub fn handle_large_output(
        &self,
        content: &str,
        server_name: &str,
        tool_name: &str,
    ) -> Result<String, String> {
        if !self.needs_handling(content) {
            return Ok(content.to_string());
        }

        if !self.config.enable_persistence {
            return self.truncate_content(content);
        }

        // 持久化到文件
        let persist_result = self.persist_content(content, server_name, tool_name)?;
        
        match persist_result {
            PersistResult::Success { filepath, original_size, .. } => {
                Ok(self.generate_instructions(&filepath, original_size, OutputFormat::Text))
            }
            PersistResult::Error(e) => {
                // 持久化失败，回退到截断
                log::warn!("Failed to persist large output: {}, falling back to truncation", e);
                self.truncate_content(content)
            }
        }
    }

    /// 持久化内容到文件
    pub fn persist_content(
        &self,
        content: &str,
        server_name: &str,
        tool_name: &str,
    ) -> Result<PersistResult, String> {
        // 生成唯一文件名
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        
        let random_suffix: String = (0..6)
            .map(|_| char::from(b'a' + rand_int(26) as u8))
            .collect();
        
        let filename = format!(
            "mcp-{}-{}-{}-{}.txt",
            sanitize_name(server_name),
            sanitize_name(tool_name),
            timestamp,
            random_suffix
        );
        
        let filepath = self.config.output_dir.join(&filename);
        
        // 写入文件
        match fs::write(&filepath, content) {
            Ok(_) => {
                let original_size = content.len();
                let persisted_size = original_size; // 同样大小
                
                // 清理旧文件
                let _ = self.cleanup_old_files();
                
                Ok(PersistResult::Success {
                    filepath,
                    original_size,
                    persisted_size,
                })
            }
            Err(e) => Ok(PersistResult::Error(format!("Failed to write file: {}", e))),
        }
    }

    /// 截断内容
    pub fn truncate_content(&self, content: &str) -> Result<String, String> {
        if content.len() <= self.config.max_output_size {
            return Ok(content.to_string());
        }

        let truncated = &content[..self.config.max_output_size];
        let remaining = content.len() - self.config.max_output_size;
        
        Ok(format!(
            "{}\n\n... [输出已截断，剩余 {} 字符被省略]\n\n使用分页或过滤工具获取特定数据部分。",
            truncated,
            remaining
        ))
    }

    /// 生成读取指令
    pub fn generate_instructions(
        &self,
        filepath: &Path,
        original_size: usize,
        format: OutputFormat,
    ) -> String {
        let format_desc = match format {
            OutputFormat::Json => "JSON 格式",
            OutputFormat::Text => "文本格式",
            OutputFormat::ToolResult => "工具结果格式",
            OutputFormat::StructuredContent => "结构化内容",
            OutputFormat::ContentArray => "内容数组",
        };

        let size_mb = original_size as f64 / (1024.0 * 1024.0);
        
        format!(
            r#"大型输出已保存到文件

文件路径: {}
原始大小: {:.2} MB ({} 字符)
格式: {}

要查看此输出:
1. 使用文件读取工具打开文件
2. 或在终端运行: cat "{}"

如果此 MCP 服务器提供分页或过滤工具，请使用它们获取特定数据部分。"#,
            filepath.display(),
            size_mb,
            original_size,
            format_desc,
            filepath.display()
        )
    }

    /// 清理旧文件
    fn cleanup_old_files(&self) -> Result<(), String> {
        let entries: Vec<_> = fs::read_dir(&self.config.output_dir)
            .map_err(|e| format!("Failed to read directory: {}", e))?
            .filter_map(|e| e.ok())
            .collect();
        
        if entries.len() <= self.config.max_files {
            return Ok(());
        }
        
        // 按修改时间排序，删除最旧的文件
        let mut entries_with_time: Vec<_> = entries
            .iter()
            .filter_map(|e| {
                let meta = e.metadata().ok()?;
                let time = meta.modified().ok()?;
                Some((e.path(), time))
            })
            .collect();
        
        entries_with_time.sort_by_key(|(_, t)| *t);
        
        // 删除超出限制的旧文件
        let to_remove = entries_with_time.len().saturating_sub(self.config.max_files);
        for (path, _) in entries_with_time.into_iter().take(to_remove) {
            let _ = fs::remove_file(path);
        }
        
        Ok(())
    }
}

/// 辅助函数：生成随机整数
fn rand_int(max: u32) -> u32 {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    
    let state = RandomState::new();
    let mut hasher = state.build_hasher();
    hasher.write_u64(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0));
    hasher.finish() as u32 % max
}

/// 辅助函数：清理名称用于文件名
fn sanitize_name(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_name() {
        assert_eq!(sanitize_name("my-server"), "my-server");
        assert_eq!(sanitize_name("my server"), "my-server");
        assert_eq!(sanitize_name("my@server!"), "my-server");
    }

    #[test]
    fn test_estimate_tokens() {
        let content = "a".repeat(1000);
        let tokens = LargeOutputHandler::estimate_tokens(&content);
        assert_eq!(tokens, 250); // 1000 / 4
    }

    #[test]
    fn test_needs_handling() {
        let config = LargeOutputConfig {
            max_output_size: 100,
            ..Default::default()
        };
        let handler = LargeOutputHandler::new(config);
        
        assert!(!handler.needs_handling("small"));
        assert!(handler.needs_handling(&"x".repeat(101)));
    }

    #[test]
    fn test_truncate_content() {
        let config = LargeOutputConfig {
            max_output_size: 10,
            ..Default::default()
        };
        let handler = LargeOutputHandler::new(config);
        
        let content = "1234567890extra";
        let result = handler.truncate_content(content).unwrap();
        
        assert!(result.starts_with("1234567890"));
        assert!(result.contains("截断"));
    }
}
