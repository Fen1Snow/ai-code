// Bundled Skills 注册表 - 内置 Skills 管理
// 实现 TypeScript 参考：skills/bundledSkills.ts, skills/loadSkillsDir.ts

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::fs;
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

/// Skill 执行上下文
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SkillContext {
    /// 内联执行
    #[serde(rename = "inline")]
    Inline,
    /// 分支执行
    #[serde(rename = "fork")]
    Fork,
}

impl Default for SkillContext {
    fn default() -> Self {
        Self::Inline
    }
}

/// Hooks 设置 (简化版本)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HooksSettings {
    #[serde(default)]
    pub pre_tool_use: Option<Vec<String>>,
    #[serde(default)]
    pub post_tool_use: Option<Vec<String>>,
    #[serde(default)]
    pub notification: Option<Vec<String>>,
    #[serde(default)]
    pub stop: Option<Vec<String>>,
}

/// Skill 文件定义
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillFile {
    /// 相对路径
    pub path: String,
    /// 文件内容
    pub content: String,
}

/// 内置 Skill 定义 - 完整版本，对齐 TypeScript BundledSkillDefinition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundledSkill {
    // === 基础字段 ===
    /// Skill 名称
    pub name: String,
    /// 描述
    pub description: String,
    
    // === TypeScript 对齐字段 ===
    /// 别名列表
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aliases: Option<Vec<String>>,
    
    /// 使用时机说明
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when_to_use: Option<String>,
    
    /// 参数提示
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub argument_hint: Option<String>,
    
    /// 允许使用的工具列表
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_tools: Option<Vec<String>>,
    
    /// 指定模型
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    
    /// 是否禁用模型调用
    #[serde(default)]
    pub disable_model_invocation: bool,
    
    /// 用户是否可调用
    #[serde(default = "default_user_invocable")]
    pub user_invocable: bool,
    
    /// 动态启用检查 (序列化为字符串表示)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_enabled_condition: Option<String>,
    
    /// Hooks 设置
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hooks: Option<HooksSettings>,
    
    /// 执行上下文
    #[serde(default)]
    pub context: SkillContext,
    
    /// 代理名称
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    
    /// 附加文件 (路径 -> 内容)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub files: Option<BTreeMap<String, String>>,
    
    // === Rust 扩展字段 ===
    /// 分类
    #[serde(default)]
    pub category: String,
    
    /// 版本
    #[serde(default)]
    pub version: String,
    
    /// 指令内容
    #[serde(default)]
    pub instructions: String,
    
    /// 示例
    #[serde(default)]
    pub examples: Vec<String>,
    
    /// 标签
    #[serde(default)]
    pub tags: Vec<String>,
    
    /// 来源
    #[serde(default)]
    pub source: SkillSource,
    
    /// 加载来源路径
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loaded_from: Option<String>,
    
    /// 内容长度
    #[serde(default)]
    pub content_length: usize,
    
    /// 是否隐藏
    #[serde(default)]
    pub is_hidden: bool,
    
    /// 进度消息
    #[serde(default)]
    pub progress_message: String,
}

fn default_user_invocable() -> bool {
    true
}

/// Skill 来源
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub enum SkillSource {
    #[default]
    #[serde(rename = "bundled")]
    Bundled,
    #[serde(rename = "disk")]
    Disk,
    #[serde(rename = "mcp")]
    Mcp,
    #[serde(rename = "user")]
    User,
}

impl BundledSkill {
    /// 创建新的 Skill
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            aliases: None,
            when_to_use: None,
            argument_hint: None,
            allowed_tools: None,
            model: None,
            disable_model_invocation: false,
            user_invocable: true,
            is_enabled_condition: None,
            hooks: None,
            context: SkillContext::Inline,
            agent: None,
            files: None,
            category: "general".to_string(),
            version: "1.0.0".to_string(),
            instructions: String::new(),
            examples: Vec::new(),
            tags: Vec::new(),
            source: SkillSource::Bundled,
            loaded_from: None,
            content_length: 0,
            is_hidden: false,
            progress_message: "running".to_string(),
        }
    }

    /// 设置别名
    pub fn with_aliases(mut self, aliases: Vec<String>) -> Self {
        self.aliases = Some(aliases);
        self
    }

    /// 设置使用时机
    pub fn with_when_to_use(mut self, when_to_use: impl Into<String>) -> Self {
        self.when_to_use = Some(when_to_use.into());
        self
    }

    /// 设置参数提示
    pub fn with_argument_hint(mut self, hint: impl Into<String>) -> Self {
        self.argument_hint = Some(hint.into());
        self
    }

    /// 设置允许的工具
    pub fn with_allowed_tools(mut self, tools: Vec<String>) -> Self {
        self.allowed_tools = Some(tools);
        self
    }

    /// 设置模型
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// 设置禁用模型调用
    pub fn with_disable_model_invocation(mut self, disable: bool) -> Self {
        self.disable_model_invocation = disable;
        self
    }

    /// 设置用户可调用
    pub fn with_user_invocable(mut self, invocable: bool) -> Self {
        self.user_invocable = invocable;
        self.is_hidden = !invocable;
        self
    }

    /// 设置 hooks
    pub fn with_hooks(mut self, hooks: HooksSettings) -> Self {
        self.hooks = Some(hooks);
        self
    }

    /// 设置上下文
    pub fn with_context(mut self, context: SkillContext) -> Self {
        self.context = context;
        self
    }

    /// 设置代理
    pub fn with_agent(mut self, agent: impl Into<String>) -> Self {
        self.agent = Some(agent.into());
        self
    }

    /// 设置附加文件
    pub fn with_files(mut self, files: BTreeMap<String, String>) -> Self {
        self.files = Some(files);
        self
    }

    /// 设置指令
    pub fn with_instructions(mut self, instructions: impl Into<String>) -> Self {
        self.instructions = instructions.into();
        self.content_length = self.instructions.len();
        self
    }

    /// 设置示例
    pub fn with_examples(mut self, examples: Vec<String>) -> Self {
        self.examples = examples;
        self
    }

    /// 设置标签
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    /// 设置分类
    pub fn with_category(mut self, category: impl Into<String>) -> Self {
        self.category = category.into();
        self
    }

    /// 检查是否启用
    pub fn is_enabled(&self) -> bool {
        // 如果有条件，需要外部评估器检查
        // 目前简化为始终返回 true
        self.is_enabled_condition.is_none() || self.user_invocable
    }

    /// 获取提示内容
    pub fn get_prompt(&self, args: Option<&str>) -> String {
        let mut prompt = self.instructions.clone();
        if let Some(a) = args {
            prompt = prompt.replace("{{args}}", a);
        }
        prompt
    }
}

/// Prompt 生成器 trait
#[allow(dead_code)]
pub trait PromptGenerator: Send + Sync {
    fn generate(&self, args: &str, context: &SkillExecutionContext) -> Vec<ContentBlock>;
}

/// Skill 执行上下文
#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct SkillExecutionContext {
    pub working_directory: Option<PathBuf>,
    pub environment: BTreeMap<String, String>,
    pub skill_root: Option<PathBuf>,
}

/// 内容块类型
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContentBlock {
    Text { text: String },
    Image { source: ImageSource },
    ToolUse { id: String, name: String, input: JsonValue },
}

/// 图片源
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageSource {
    #[serde(rename = "type")]
    pub source_type: String,
    pub media_type: String,
    pub data: String,
}

/// 内置 Skills 注册表
pub struct BundledSkillsRegistry {
    skills: BTreeMap<String, BundledSkill>,
    aliases: BTreeMap<String, String>,
    categories: BTreeMap<String, Vec<String>>,
    #[allow(dead_code)]
    prompt_generators: BTreeMap<String, Arc<dyn PromptGenerator>>,
}

impl BundledSkillsRegistry {
    /// 创建新的注册表
    pub fn new() -> Self {
        let mut registry = Self {
            skills: BTreeMap::new(),
            aliases: BTreeMap::new(),
            categories: BTreeMap::new(),
            prompt_generators: BTreeMap::new(),
        };
        registry.register_default_skills();
        registry
    }

    /// 注册默认内置 Skills
    fn register_default_skills(&mut self) {
        // Debug Skill
        self.register(BundledSkill::new("debug", "Debug and troubleshoot issues")
            .with_category("development")
            .with_instructions(r#"# Debug Skill
Analyze errors, check logs, identify root causes, and suggest fixes.
Usage: "Debug this error: ..." "#)
            .with_examples(vec!["Debug this compilation error".into()])
            .with_tags(vec!["debug".into(), "troubleshoot".into()]));

        // Simplify Skill
        self.register(BundledSkill::new("simplify", "Simplify complex code or explanations")
            .with_category("writing")
            .with_instructions(r#"# Simplify Skill
Reduce complexity, improve readability, remove redundancy.
Usage: "Simplify this: ..." "#)
            .with_tags(vec!["simplify".into(), "clarity".into()]));

        // Verify Skill
        self.register(BundledSkill::new("verify", "Verify code correctness or claim accuracy")
            .with_category("quality")
            .with_instructions(r#"# Verify Skill
Check correctness, validate claims, run tests.
Usage: "Verify this code: ..." "#)
            .with_tags(vec!["verify".into(), "test".into()]));
    }

    /// 注册 Skill
    pub fn register(&mut self, skill: BundledSkill) {
        if let Some(ref aliases) = skill.aliases {
            for alias in aliases {
                self.aliases.insert(alias.clone(), skill.name.clone());
            }
        }
        
        let category = skill.category.clone();
        let name = skill.name.clone();
        
        self.skills.insert(name.clone(), skill);
        
        self.categories.entry(category)
            .or_default()
            .push(name);
    }

    /// 按名称获取 Skill
    pub fn get(&self, name: &str) -> Option<&BundledSkill> {
        self.skills.get(name)
            .or_else(|| {
                self.aliases.get(name)
                    .and_then(|real_name| self.skills.get(real_name))
            })
    }

    /// 列出所有 Skill 名称
    pub fn list_names(&self) -> Vec<&String> {
        self.skills.keys().collect()
    }

    /// 按分类列出 Skills
    pub fn list_by_category(&self, category: &str) -> Vec<&BundledSkill> {
        self.categories.get(category)
            .map(|names| names.iter().filter_map(|n| self.skills.get(n)).collect())
            .unwrap_or_default()
    }

    /// 获取用户可调用的 Skills
    pub fn list_user_invocable(&self) -> Vec<&BundledSkill> {
        self.skills.values()
            .filter(|s| s.user_invocable && s.is_enabled())
            .collect()
    }
}

impl Default for BundledSkillsRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// 文件提取器 - 提取 Skill 附加文件到磁盘
#[allow(dead_code)]
pub struct SkillFileExtractor {
    base_dir: PathBuf,
}

#[allow(dead_code)]
impl SkillFileExtractor {
    pub fn new(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    /// 提取 Skill 文件到磁盘
    pub fn extract(&self, skill: &BundledSkill) -> Result<PathBuf, std::io::Error> {
        if let Some(ref files) = skill.files {
            let skill_dir = self.base_dir.join(".skill_files").join(&skill.name);
            fs::create_dir_all(&skill_dir)?;
            
            for (path, content) in files {
                let file_path = skill_dir.join(path);
                if let Some(parent) = file_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::write(&file_path, content)?;
            }
            
            return Ok(skill_dir);
        }
        
        Ok(self.base_dir.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skill_builder() {
        let skill = BundledSkill::new("test", "Test skill")
            .with_aliases(vec!["t".to_string()])
            .with_allowed_tools(vec!["Read".to_string(), "Write".to_string()])
            .with_user_invocable(true);
        
        assert_eq!(skill.name, "test");
        assert_eq!(skill.aliases, Some(vec!["t".to_string()]));
        assert!(skill.user_invocable);
    }

    #[test]
    fn test_registry() {
        let registry = BundledSkillsRegistry::new();
        
        assert!(registry.get("debug").is_some());
        assert!(registry.get("simplify").is_some());
        assert!(registry.get("verify").is_some());
    }

    #[test]
    fn test_user_invocable_filter() {
        let registry = BundledSkillsRegistry::new();
        let invocable = registry.list_user_invocable();
        
        assert!(!invocable.is_empty());
        assert!(invocable.iter().all(|s| s.user_invocable));
    }
}
