// Load Skills Directory - 从磁盘加载 Skills
// 完整移植 TypeScript 参考：skills/loadSkillsDir.ts (34KB)

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::fs;
use std::time::SystemTime;
use serde::{Deserialize, Serialize};
use regex::Regex;

use crate::bundled_skills::{BundledSkill, SkillContext, SkillSource};

/// Skill Frontmatter 元数据
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SkillFrontmatter {
    /// Skill 名称
    #[serde(default)]
    pub name: String,
    
    /// 描述
    #[serde(default)]
    pub description: String,
    
    /// 别名
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aliases: Option<Vec<String>>,
    
    /// 使用时机
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when_to_use: Option<String>,
    
    /// 参数提示
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub argument_hint: Option<String>,
    
    /// 允许的工具
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_tools: Option<Vec<String>>,
    
    /// 模型
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    
    /// 禁用模型调用
    #[serde(default)]
    pub disable_model_invocation: bool,
    
    /// 用户可调用
    #[serde(default = "default_true")]
    pub user_invocable: bool,
    
    /// 隐藏
    #[serde(default)]
    pub is_hidden: bool,
    
    /// 上下文
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    
    /// 代理
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    
    /// 版本
    #[serde(default)]
    pub version: String,
    
    /// 标签
    #[serde(default)]
    pub tags: Vec<String>,
}

fn default_true() -> bool { true }

/// 解析后的 Skill 文件
#[derive(Debug, Clone)]
pub struct ParsedSkillFile {
    /// Frontmatter 元数据
    pub frontmatter: SkillFrontmatter,
    /// 内容主体
    pub body: String,
    /// 文件路径
    pub path: PathBuf,
    /// 修改时间
    #[allow(dead_code)]
    pub modified: Option<SystemTime>,
}

/// Skill 加载器
pub struct SkillLoader {
    skills_dir: PathBuf,
    cache: BTreeMap<String, (BundledSkill, SystemTime)>,
}

impl SkillLoader {
    /// 创建新的 Skill 加载器
    pub fn new(skills_dir: PathBuf) -> Self {
        Self {
            skills_dir,
            cache: BTreeMap::new(),
        }
    }

    /// 加载所有 Skills
    pub fn load_all(&mut self) -> Result<Vec<BundledSkill>, SkillLoadError> {
        let mut skills = Vec::new();
        
        if !self.skills_dir.exists() {
            return Ok(skills);
        }
        
        for entry in fs::read_dir(&self.skills_dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_dir() {
                // 递归加载子目录
                if let Ok(sub_skills) = self.load_dir(&path) {
                    skills.extend(sub_skills);
                }
            } else if is_skill_file(&path) {
                // 加载单个文件
                if let Ok(skill) = self.load_file(&path) {
                    skills.push(skill);
                }
            }
        }
        
        Ok(skills)
    }

    /// 加载目录中的 Skills
    fn load_dir(&mut self, dir: &Path) -> Result<Vec<BundledSkill>, SkillLoadError> {
        let mut skills = Vec::new();
        
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_dir() {
                if let Ok(sub_skills) = self.load_dir(&path) {
                    skills.extend(sub_skills);
                }
            } else if is_skill_file(&path) {
                if let Ok(skill) = self.load_file(&path) {
                    skills.push(skill);
                }
            }
        }
        
        Ok(skills)
    }

    /// 加载单个 Skill 文件
    fn load_file(&mut self, path: &Path) -> Result<BundledSkill, SkillLoadError> {
        let content = fs::read_to_string(path)?;
        let parsed = parse_skill_file(&content, path)?;
        
        let skill = create_skill_from_parsed(parsed)?;
        Ok(skill)
    }

    /// 热重载 - 检查修改并重新加载
    pub fn hot_reload(&mut self) -> Result<Vec<BundledSkill>, SkillLoadError> {
        let mut changed = Vec::new();
        
        for entry in fs::read_dir(&self.skills_dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if is_skill_file(&path) {
                let metadata = fs::metadata(&path)?;
                let modified = metadata.modified().ok();
                
                if let Some(mtime) = modified {
                    let needs_reload = self.cache.get(path.to_str().unwrap_or(""))
                        .map(|(_, cached_mtime)| mtime > *cached_mtime)
                        .unwrap_or(true);
                    
                    if needs_reload {
                        if let Ok(skill) = self.load_file(&path) {
                            self.cache.insert(
                                path.to_str().unwrap_or("").to_string(),
                                (skill.clone(), mtime)
                            );
                            changed.push(skill);
                        }
                    }
                }
            }
        }
        
        Ok(changed)
    }

    /// 获取 Skills 目录
    pub fn skills_dir(&self) -> &Path {
        &self.skills_dir
    }
}

/// 判断是否为 Skill 文件
fn is_skill_file(path: &Path) -> bool {
    let ext = path.extension().unwrap_or_default().to_str().unwrap_or("");
    ext == "md" || ext == "txt" || ext == "skill"
}

/// 解析 Skill 文件
pub fn parse_skill_file(content: &str, path: &Path) -> Result<ParsedSkillFile, SkillLoadError> {
    let (frontmatter, body) = parse_frontmatter(content)?;
    
    let metadata = fs::metadata(path).ok();
    let modified = metadata.and_then(|m| m.modified().ok());
    
    Ok(ParsedSkillFile {
        frontmatter,
        body,
        path: path.to_path_buf(),
        modified,
    })
}

/// 解析 Frontmatter
pub fn parse_frontmatter(content: &str) -> Result<(SkillFrontmatter, String), SkillLoadError> {
    // 支持 YAML 和 TOML frontmatter
    let yaml_pattern = Regex::new(r"^---\s*\n([\s\S]*?)\n---\s*\n").unwrap();
    let toml_pattern = Regex::new(r"^\+\+\+\s*\n([\s\S]*?)\n\+\+\+\s*\n").unwrap();
    
    if let Some(caps) = yaml_pattern.captures(content) {
        let fm_str = caps.get(1).unwrap().as_str();
        let body = yaml_pattern.replace(content, "").to_string();
        let frontmatter: SkillFrontmatter = serde_yaml::from_str(fm_str)
            .map_err(|e| SkillLoadError::ParseError(format!("YAML parse error: {}", e)))?;
        return Ok((frontmatter, body));
    }
    
    if let Some(caps) = toml_pattern.captures(content) {
        let fm_str = caps.get(1).unwrap().as_str();
        let body = toml_pattern.replace(content, "").to_string();
        let frontmatter: SkillFrontmatter = toml::from_str(fm_str)
            .map_err(|e| SkillLoadError::ParseError(format!("TOML parse error: {}", e)))?;
        return Ok((frontmatter, body));
    }
    
    // 无 frontmatter，使用默认值
    Ok((SkillFrontmatter::default(), content.to_string()))
}

/// 从解析结果创建 Skill
fn create_skill_from_parsed(parsed: ParsedSkillFile) -> Result<BundledSkill, SkillLoadError> {
    let fm = parsed.frontmatter;
    
    // 从文件名推断名称
    let name = if fm.name.is_empty() {
        parsed.path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string()
    } else {
        fm.name
    };
    
    // 解析上下文
    let context = fm.context.as_ref()
        .and_then(|c| match c.as_str() {
            "inline" => Some(SkillContext::Inline),
            "fork" => Some(SkillContext::Fork),
            _ => None,
        })
        .unwrap_or_default();
    
    let skill = BundledSkill::new(&name, &fm.description)
        .with_aliases(fm.aliases.unwrap_or_default())
        .with_instructions(parsed.body)
        .with_context(context)
        .with_user_invocable(fm.user_invocable);
    
    // 使用 builder 模式设置可选字段
    let mut skill = skill;
    
    // 设置可选字段
    if let Some(when_to_use) = fm.when_to_use {
        skill = skill.with_when_to_use(when_to_use);
    }
    if let Some(argument_hint) = fm.argument_hint {
        skill = skill.with_argument_hint(argument_hint);
    }
    if let Some(allowed_tools) = fm.allowed_tools {
        skill = skill.with_allowed_tools(allowed_tools);
    }
    if let Some(model) = fm.model {
        skill = skill.with_model(model);
    }
    if let Some(agent) = fm.agent {
        skill = skill.with_agent(agent);
    }
    if !fm.tags.is_empty() {
        skill = skill.with_tags(fm.tags);
    }
    
    // 设置来源和路径
    skill.loaded_from = parsed.path.to_str().map(|s| s.to_string());
    skill.source = SkillSource::Disk;
    skill.is_hidden = fm.is_hidden || !fm.user_invocable;
    
    Ok(skill)
}

/// Skill 加载错误
#[derive(Debug)]
pub enum SkillLoadError {
    /// IO 错误
    IoError(std::io::Error),
    /// 解析错误
    ParseError(String),
    /// 无效文件
    InvalidFile(String),
}

impl From<std::io::Error> for SkillLoadError {
    fn from(e: std::io::Error) -> Self {
        Self::IoError(e)
    }
}

impl std::fmt::Display for SkillLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IoError(e) => write!(f, "IO error: {}", e),
            Self::ParseError(s) => write!(f, "Parse error: {}", s),
            Self::InvalidFile(s) => write!(f, "Invalid file: {}", s),
        }
    }
}

impl std::error::Error for SkillLoadError {}

/// 创建 Skill 命令
pub fn create_skill_command(skill: &BundledSkill) -> SkillCommand {
    SkillCommand {
        name: skill.name.clone(),
        description: skill.description.clone(),
        aliases: skill.aliases.clone().unwrap_or_default(),
        instructions: skill.instructions.clone(),
        allowed_tools: skill.allowed_tools.clone().unwrap_or_default(),
        user_invocable: skill.user_invocable,
    }
}

/// Skill 命令结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillCommand {
    pub name: String,
    pub description: String,
    pub aliases: Vec<String>,
    pub instructions: String,
    pub allowed_tools: Vec<String>,
    pub user_invocable: bool,
}

/// 解析 Skill Frontmatter 字段
#[allow(dead_code)]
pub fn parse_skill_frontmatter_fields(content: &str) -> Result<SkillFrontmatter, SkillLoadError> {
    let (fm, _) = parse_frontmatter(content)?;
    Ok(fm)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_parse_yaml_frontmatter() {
        let content = r#"---
name: test-skill
description: A test skill
aliases:
  - ts
  - test
allowed_tools:
  - Read
  - Write
---

# Test Skill

This is the body.
"#;
        let (fm, body) = parse_frontmatter(content).unwrap();
        assert_eq!(fm.name, "test-skill");
        assert_eq!(fm.description, "A test skill");
        assert_eq!(fm.aliases, Some(vec!["ts".to_string(), "test".to_string()]));
        assert!(body.contains("# Test Skill"));
    }

    #[test]
    fn test_parse_toml_frontmatter() {
        let content = r#"+++
name = "toml-skill"
description = "A TOML skill"
user_invocable = true
+++

# TOML Skill Body
"#;
        let (fm, body) = parse_frontmatter(content).unwrap();
        assert_eq!(fm.name, "toml-skill");
        assert_eq!(fm.description, "A TOML skill");
        assert!(body.contains("# TOML Skill Body"));
    }

    #[test]
    fn test_no_frontmatter() {
        let content = "# Just a regular skill\n\nNo frontmatter here.";
        let (fm, body) = parse_frontmatter(content).unwrap();
        assert!(fm.name.is_empty());
        assert_eq!(body, content);
    }

    #[test]
    fn test_skill_loader() {
        let temp_dir = TempDir::new().unwrap();
        let skill_path = temp_dir.path().join("test.md");
        
        let content = r#"---
name: test
description: Test
---
Body"#;
        fs::write(&skill_path, content).unwrap();
        
        let mut loader = SkillLoader::new(temp_dir.path().to_path_buf());
        let skills = loader.load_all().unwrap();
        
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "test");
    }

    #[test]
    fn test_create_skill_command() {
        let skill = BundledSkill::new("test", "Test skill")
            .with_aliases(vec!["t".to_string()])
            .with_allowed_tools(vec!["Read".to_string()])
            .with_instructions("Test instructions");
        
        let cmd = create_skill_command(&skill);
        assert_eq!(cmd.name, "test");
        assert_eq!(cmd.aliases, vec!["t"]);
        assert_eq!(cmd.allowed_tools, vec!["Read"]);
    }
}
