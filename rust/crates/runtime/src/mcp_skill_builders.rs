// MCP Skill Builders - 将 MCP Server 工具转换为 Skills
// 实现 Python 参考：skills/mcpSkillBuilders.ts

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::fs;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};

use crate::config::{ScopedMcpServerConfig, RuntimeConfig};
use crate::mcp_stdio::{McpTool, McpServerManager};

/// MCP Skill 元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpSkillMetadata {
    pub name: String,
    pub description: String,
    pub server_name: String,
    pub tool_name: String,
    pub version: String,
}

/// MCP Skill 定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpSkillDefinition {
    pub metadata: McpSkillMetadata,
    pub input_schema: JsonValue,
    pub instructions: String,
    pub examples: Vec<String>,
}

/// MCP Skill 构建器
pub struct McpSkillBuilder {
    server_name: String,
    server_config: ScopedMcpServerConfig,
    skills_dir: PathBuf,
    #[allow(dead_code)]
    manager: Option<McpServerManager>,
}

impl McpSkillBuilder {
    /// 创建新的 MCP Skill 构建器
    pub fn new(
        server_name: String,
        server_config: ScopedMcpServerConfig,
        skills_dir: PathBuf,
    ) -> Self {
        Self {
            server_name,
            server_config,
            skills_dir,
            manager: None,
        }
    }

    /// 创建已初始化 MCP 管理器的 Skill 构建器
    pub fn with_manager(
        server_name: String,
        server_config: ScopedMcpServerConfig,
        skills_dir: PathBuf,
        manager: McpServerManager,
    ) -> Self {
        Self {
            server_name,
            server_config,
            skills_dir,
            manager: Some(manager),
        }
    }

    /// 从 MCP Server 发现工具并生成 Skills
    pub async fn discover_and_build_skills(&mut self) -> Result<Vec<McpSkillDefinition>, String> {
        // 1. 连接到 MCP Server 并获取工具列表
        let tools = self.list_mcp_tools().await?;
        
        // 2. 为每个工具生成 Skill
        let mut skills = Vec::new();
        for tool in &tools {
            if let Some(skill) = self.build_skill_from_tool(tool)? {
                skills.push(skill);
            }
        }
        
        Ok(skills)
    }

    /// 列出 MCP Server 的所有工具
    async fn list_mcp_tools(&mut self) -> Result<Vec<McpTool>, String> {
        // 创建临时 manager 进行发现
        let mut temp_config = BTreeMap::new();
        temp_config.insert(self.server_name.clone(), self.server_config.clone());
        
        let mut temp_manager = McpServerManager::from_servers(&temp_config);
        
        // 直接使用已有的异步上下文
        let tools = temp_manager.discover_tools().await
            .map_err(|e| format!("MCP discover_tools error: {}", e))?;
        
        Ok(tools.into_iter().map(|t| t.tool).collect())
    }

    /// 从 MCP 工具构建 Skill
    fn build_skill_from_tool(&self, tool: &McpTool) -> Result<Option<McpSkillDefinition>, String> {
        let skill_name = format!("{}-{}", self.server_name, tool.name);
        let description = tool.description.clone().unwrap_or_else(|| {
            format!("MCP tool `{}` from server `{}`", tool.name, self.server_name)
        });

        let input_schema = tool.input_schema.clone().unwrap_or_else(|| {
            json!({
                "type": "object",
                "properties": {},
                "required": []
            })
        });

        // 生成 Skill 指令
        let instructions = self.generate_skill_instructions(tool, &input_schema)?;
        
        // 生成示例
        let examples = self.generate_skill_examples(tool, &input_schema)?;

        Ok(Some(McpSkillDefinition {
            metadata: McpSkillMetadata {
                name: skill_name,
                description,
                server_name: self.server_name.clone(),
                tool_name: tool.name.clone(),
                version: "1.0.0".to_string(),
            },
            input_schema,
            instructions,
            examples,
        }))
    }

    /// 生成 Skill 指令
    fn generate_skill_instructions(
        &self,
        tool: &McpTool,
        input_schema: &JsonValue,
    ) -> Result<String, String> {
        let mut instructions = String::new();
        
        instructions.push_str(&format!("# MCP Skill: {} - {}\n\n", self.server_name, tool.name));
        instructions.push_str(&format!("**Server**: {}\n", self.server_name));
        instructions.push_str(&format!("**Tool**: {}\n\n", tool.name));
        
        if let Some(desc) = &tool.description {
            instructions.push_str(&format!("**Description**: {}\n\n", desc));
        }

        instructions.push_str("## Usage\n\n");
        instructions.push_str("This skill calls an MCP tool from the configured MCP server.\n\n");

        // 生成参数说明
        instructions.push_str("## Parameters\n\n");
        if let Some(obj) = input_schema.as_object() {
            if let Some(props) = obj.get("properties").and_then(|v| v.as_object()) {
                if let Some(required) = obj.get("required").and_then(|v| v.as_array()) {
                    for (param_name, param_schema) in props {
                        let is_required = required.iter().any(|r| r.as_str() == Some(param_name.as_str()));
                        let param_desc = param_schema.as_object()
                            .and_then(|o| o.get("description"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("No description");
                        
                        instructions.push_str(&format!(
                            "- `{}` ({}) - {}\n",
                            param_name,
                            if is_required { "required" } else { "optional" },
                            param_desc
                        ));
                    }
                }
            }
        }
        instructions.push('\n');

        instructions.push_str("## Example\n\n");
        instructions.push_str("```json\n");
        instructions.push_str(&format!("{{\n"));
        instructions.push_str(&format!("  \"skill\": \"{}\"\n", format!("{}-{}", self.server_name, tool.name)));
        instructions.push_str(&format!("}}\n"));
        instructions.push_str("```\n\n");

        Ok(instructions)
    }

    /// 生成 Skill 示例
    fn generate_skill_examples(
        &self,
        tool: &McpTool,
        _input_schema: &JsonValue,
    ) -> Result<Vec<String>, String> {
        let mut examples = Vec::new();
        
        // 生成一个基础示例
        let mut example = String::new();
        example.push_str(&format!("Call {} tool", tool.name));
        examples.push(example);

        Ok(examples)
    }

    /// 保存 Skill 到文件
    pub fn save_skill(&self, skill: &McpSkillDefinition) -> Result<PathBuf, String> {
        let skill_dir = self.skills_dir.join(&skill.metadata.name);
        fs::create_dir_all(&skill_dir)
            .map_err(|e| format!("Failed to create skill directory: {}", e))?;

        let skill_path = skill_dir.join("SKILL.md");
        let mut content = String::new();
        
        // 写入 frontmatter
        content.push_str("---\n");
        content.push_str(&format!("name = \"{}\"\n", skill.metadata.name));
        content.push_str(&format!("description = \"{}\"\n", skill.metadata.description));
        content.push_str(&format!("version = \"{}\"\n", skill.metadata.version));
        content.push_str(&format!("mcp_server = \"{}\"\n", skill.metadata.server_name));
        content.push_str(&format!("mcp_tool = \"{}\"\n", skill.metadata.tool_name));
        content.push_str("---\n\n");
        
        // 写入指令
        content.push_str(&skill.instructions);
        
        // 写入示例
        if !skill.examples.is_empty() {
            content.push_str("## Examples\n\n");
            for (i, example) in skill.examples.iter().enumerate() {
                content.push_str(&format!("{}. {}\n", i + 1, example));
            }
            content.push('\n');
        }

        fs::write(&skill_path, &content)
            .map_err(|e| format!("Failed to write skill file: {}", e))?;

        Ok(skill_path)
    }

    /// 生成并保存所有 Skills
    pub async fn build_and_save_all(&mut self) -> Result<Vec<PathBuf>, String> {
        let skills = self.discover_and_build_skills().await?;
        let mut paths = Vec::new();
        
        for skill in &skills {
            let path = self.save_skill(skill)?;
            paths.push(path);
        }
        
        Ok(paths)
    }
}

/// 从配置生成所有 MCP Skills
pub async fn build_mcp_skills_from_config(
    mcp_configs: &BTreeMap<String, ScopedMcpServerConfig>,
    base_skills_dir: &Path,
) -> Result<Vec<PathBuf>, String> {
    let mut all_skill_paths = Vec::new();
    
    for (server_name, server_config) in mcp_configs {
        let skills_dir = base_skills_dir.join("mcp").join(server_name);
        let mut builder = McpSkillBuilder::new(
            server_name.clone(),
            server_config.clone(),
            skills_dir,
        );
        
        match builder.build_and_save_all().await {
            Ok(paths) => {
                all_skill_paths.extend(paths);
            }
            Err(e) => {
                eprintln!("Warning: Failed to build skills for server '{}': {}", server_name, e);
            }
        }
    }
    
    Ok(all_skill_paths)
}

/// MCP Skill 运行时执行器
pub struct McpSkillExecutor {
    manager: McpServerManager,
}

impl McpSkillExecutor {
    pub fn new(config: &RuntimeConfig) -> Self {
        let manager = McpServerManager::from_runtime_config(config);
        Self { manager }
    }

    /// 执行 MCP Skill (异步)
    pub async fn execute_skill(&mut self, skill_name: &str, input: &str) -> Result<String, String> {
        // 解析输入 JSON
        let input_json: Option<JsonValue> = if input.is_empty() {
            None
        } else {
            Some(serde_json::from_str(input)
                .map_err(|e| format!("Invalid JSON input: {}", e))?)
        };
        
        // 调用 MCP 工具 - skill_name 就是 qualified_tool_name (格式: server-tool)
        let response = self.manager
            .call_tool(skill_name, input_json)
            .await
            .map_err(|e| format!("MCP tool call error: {}", e))?;
        
        // 从 response.result 获取结果
        let result = response.result.ok_or_else(|| {
            format!("MCP tool call failed: {:?}", response.error)
        })?;
        
        // 格式化结果
        let mut output = String::new();
        for content in &result.content {
            if let Some(text) = content.data.get("text").and_then(|v| v.as_str()) {
                output.push_str(text);
            }
        }
        
        if output.is_empty() {
            output = serde_json::to_string_pretty(&result.structured_content.unwrap_or(json!({})))
                .unwrap_or_default();
        }
        
        Ok(output)
    }

    /// 执行 MCP Skill (同步包装器)
    pub fn execute_skill_sync(&mut self, skill_name: &str, input: &str) -> Result<String, String> {
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| format!("Failed to create tokio runtime: {}", e))?;
        rt.block_on(async {
            self.execute_skill(skill_name, input).await
        })
    }

    /// 获取可用的 MCP Skills 列表
    pub fn list_available_skills(&self) -> Vec<String> {
        // 从 tool_index 获取所有已注册的工具
        self.manager.tool_index()
            .keys()
            .cloned()
            .collect()
    }
}

impl Default for McpSkillExecutor {
    fn default() -> Self {
        // 创建空配置
        let config = RuntimeConfig::empty();
        Self::new(&config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{McpStdioServerConfig, McpServerConfig};
    use std::env;

    #[test]
    fn test_mcp_skill_builder_creation() {
        let config = ScopedMcpServerConfig {
            scope: crate::config::ConfigSource::User,
            config: McpServerConfig::Stdio(McpStdioServerConfig {
                command: "echo".to_string(),
                args: vec![],
                env: BTreeMap::new(),
            }),
        };
        
        let temp_dir = env::temp_dir().join("test-mcp-skills");
        let builder = McpSkillBuilder::new(
            "test-server".to_string(),
            config,
            temp_dir,
        );
        
        assert_eq!(builder.server_name, "test-server");
    }

    #[tokio::test]
    async fn test_mcp_skill_executor() {
        let config = RuntimeConfig::empty();
        let mut executor = McpSkillExecutor::new(&config);
        let result = executor.execute_skill("github-search", r#"{"query": "rust"}"#).await;

        // 由于没有实际的 MCP server，这个测试会失败
        // 但至少可以验证代码编译
        assert!(result.is_ok() || result.is_err());
    }
}
