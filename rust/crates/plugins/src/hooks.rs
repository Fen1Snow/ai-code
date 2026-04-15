use std::ffi::OsStr;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use serde_json::json;

use crate::{PluginError, PluginHooks, PluginRegistry};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookEvent {
    PreToolUse,
    PostToolUse,
}

impl HookEvent {
    fn as_str(self) -> &'static str {
        match self {
            Self::PreToolUse => "PreToolUse",
            Self::PostToolUse => "PostToolUse",
        }
    }
}

/// Hook 执行超时时间（秒）
const HOOK_TIMEOUT_SECS: u64 = 30;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HookRunResult {
    denied: bool,
    messages: Vec<String>,
    modified_input: Option<String>,
    modified_output: Option<String>,
    execution_logs: Vec<HookExecutionLog>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookExecutionLog {
    pub hook_command: String,
    pub event: HookEvent,
    pub tool_name: String,
    pub exit_code: Option<i32>,
    pub execution_time_ms: u128,
    pub output: String,
}

impl HookRunResult {
    #[must_use]
    pub fn allow(messages: Vec<String>) -> Self {
        Self {
            denied: false,
            messages,
            modified_input: None,
            modified_output: None,
            execution_logs: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_modified_input(mut self, modified_input: String) -> Self {
        self.modified_input = Some(modified_input);
        self
    }

    #[must_use]
    pub fn with_modified_output(mut self, modified_output: String) -> Self {
        self.modified_output = Some(modified_output);
        self
    }

    #[must_use]
    pub fn is_denied(&self) -> bool {
        self.denied
    }

    #[must_use]
    pub fn messages(&self) -> &[String] {
        &self.messages
    }

    #[must_use]
    pub fn modified_input(&self) -> Option<&str> {
        self.modified_input.as_deref()
    }

    #[must_use]
    pub fn modified_output(&self) -> Option<&str> {
        self.modified_output.as_deref()
    }

    #[must_use]
    pub fn execution_logs(&self) -> &[HookExecutionLog] {
        &self.execution_logs
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HookRunner {
    hooks: PluginHooks,
}

impl HookRunner {
    #[must_use]
    pub fn new(hooks: PluginHooks) -> Self {
        Self { hooks }
    }

    pub fn from_registry(plugin_registry: &PluginRegistry) -> Result<Self, PluginError> {
        Ok(Self::new(plugin_registry.aggregated_hooks()?))
    }

    #[must_use]
    pub fn run_pre_tool_use(&self, tool_name: &str, tool_input: &str) -> HookRunResult {
        self.run_commands(
            HookEvent::PreToolUse,
            &self.hooks.pre_tool_use,
            tool_name,
            tool_input,
            None,
            false,
        )
    }

    #[must_use]
    pub fn run_post_tool_use(
        &self,
        tool_name: &str,
        tool_input: &str,
        tool_output: &str,
        is_error: bool,
    ) -> HookRunResult {
        self.run_commands(
            HookEvent::PostToolUse,
            &self.hooks.post_tool_use,
            tool_name,
            tool_input,
            Some(tool_output),
            is_error,
        )
    }

    fn run_commands(
        &self,
        event: HookEvent,
        commands: &[String],
        tool_name: &str,
        tool_input: &str,
        tool_output: Option<&str>,
        is_error: bool,
    ) -> HookRunResult {
        if commands.is_empty() {
            return HookRunResult::allow(Vec::new());
        }

        let payload = json!({
            "hook_event_name": event.as_str(),
            "tool_name": tool_name,
            "tool_input": parse_tool_input(tool_input),
            "tool_input_json": tool_input,
            "tool_output": tool_output,
            "tool_result_is_error": is_error,
        })
        .to_string();

        let mut messages = Vec::new();
        let mut modified_input: Option<String> = None;
        let mut modified_output: Option<String> = None;
        let mut execution_logs = Vec::new();
        let mut current_input = tool_input.to_string();

        for command in commands {
            let start_time = std::time::Instant::now();
            match self.run_command(
                command,
                event,
                tool_name,
                &current_input,
                tool_output,
                is_error,
                &payload,
            ) {
                HookCommandOutcome::Allow { message, modified_json } => {
                    if let Some(ref msg) = message {
                        messages.push(msg.clone());
                    }
                    // 处理输入/输出重写
                    if let Some(json_str) = modified_json {
                        if event == HookEvent::PreToolUse {
                            modified_input = Some(json_str.clone());
                            current_input = json_str;
                        } else if event == HookEvent::PostToolUse {
                            modified_output = Some(json_str.clone());
                        }
                    }
                    // 记录执行日志
                    execution_logs.push(HookExecutionLog {
                        hook_command: command.to_string(),
                        event,
                        tool_name: tool_name.to_string(),
                        exit_code: Some(0),
                        execution_time_ms: start_time.elapsed().as_millis(),
                        output: message.unwrap_or_default(),
                    });
                }
                HookCommandOutcome::Deny { message } => {
                    execution_logs.push(HookExecutionLog {
                        hook_command: command.to_string(),
                        event,
                        tool_name: tool_name.to_string(),
                        exit_code: Some(2),
                        execution_time_ms: start_time.elapsed().as_millis(),
                        output: message.clone().unwrap_or_default(),
                    });
                    messages.push(message.unwrap_or_else(|| {
                        format!("{} hook denied tool `{tool_name}`", event.as_str())
                    }));
                    return HookRunResult {
                        denied: true,
                        messages,
                        modified_input,
                        modified_output,
                        execution_logs,
                    };
                }
                HookCommandOutcome::Warn { message } => {
                    execution_logs.push(HookExecutionLog {
                        hook_command: command.to_string(),
                        event,
                        tool_name: tool_name.to_string(),
                        exit_code: None,
                        execution_time_ms: start_time.elapsed().as_millis(),
                        output: message.clone(),
                    });
                    messages.push(message)
                }
            }
        }

        HookRunResult {
            denied: false,
            messages,
            modified_input,
            modified_output,
            execution_logs,
        }
    }

    #[allow(clippy::too_many_arguments, clippy::unused_self)]
    fn run_command(
        &self,
        command: &str,
        event: HookEvent,
        tool_name: &str,
        tool_input: &str,
        tool_output: Option<&str>,
        is_error: bool,
        payload: &str,
    ) -> HookCommandOutcome {
        let mut child = shell_command(command);
        child.stdin(std::process::Stdio::piped());
        child.stdout(std::process::Stdio::piped());
        child.stderr(std::process::Stdio::piped());
        child.env("HOOK_EVENT", event.as_str());
        child.env("HOOK_TOOL_NAME", tool_name);
        child.env("HOOK_TOOL_INPUT", tool_input);
        child.env("HOOK_TOOL_IS_ERROR", if is_error { "1" } else { "0" });
        if let Some(tool_output) = tool_output {
            child.env("HOOK_TOOL_OUTPUT", tool_output);
        }

        // 设置超时
        let output = match child.output_with_stdin_timeout(payload.as_bytes(), Duration::from_secs(HOOK_TIMEOUT_SECS)) {
            Ok(output) => output,
            Err(_) => {
                return HookCommandOutcome::Warn {
                    message: format!(
                        "{} hook `{command}` timed out after {}s for `{tool_name}`",
                        event.as_str(),
                        HOOK_TIMEOUT_SECS
                    ),
                };
            }
        };

        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        
        // 检查是否有修改后的 JSON 输出（以 MODIFIED_JSON: 开头）
        let (message, modified_json) = if stdout.starts_with("MODIFIED_JSON:") {
            let lines: Vec<&str> = stdout.splitn(2, '\n').collect();
            let json_str = lines.get(1).unwrap_or(&"{}").trim().to_string();
            let msg = if lines.len() > 1 && !lines[0].trim().is_empty() {
                Some(lines[0].trim().to_string())
            } else {
                None
            };
            (msg, Some(json_str))
        } else {
            ((!stdout.is_empty()).then_some(stdout), None)
        };

        match output.status.code() {
            Some(0) => HookCommandOutcome::Allow { message, modified_json },
            Some(2) => HookCommandOutcome::Deny { message },
            Some(code) => HookCommandOutcome::Warn {
                message: format_hook_warning(
                    command,
                    code,
                    message.as_deref(),
                    stderr.as_str(),
                ),
            },
            None => HookCommandOutcome::Warn {
                message: format!(
                    "{} hook `{command}` terminated by signal while handling `{tool_name}`",
                    event.as_str()
                ),
            },
        }
    }
}

enum HookCommandOutcome {
    Allow {
        message: Option<String>,
        modified_json: Option<String>,
    },
    Deny { message: Option<String> },
    Warn { message: String },
}

fn parse_tool_input(tool_input: &str) -> serde_json::Value {
    serde_json::from_str(tool_input).unwrap_or_else(|_| json!({ "raw": tool_input }))
}

fn format_hook_warning(command: &str, code: i32, stdout: Option<&str>, stderr: &str) -> String {
    let mut message =
        format!("Hook `{command}` exited with status {code}; allowing tool execution to continue");
    if let Some(stdout) = stdout.filter(|stdout| !stdout.is_empty()) {
        message.push_str(": ");
        message.push_str(stdout);
    } else if !stderr.is_empty() {
        message.push_str(": ");
        message.push_str(stderr);
    }
    message
}

fn shell_command(command: &str) -> CommandWithStdin {
    #[cfg(windows)]
    let command_builder = {
        let mut command_builder = Command::new("cmd");
        command_builder.arg("/C").arg(command);
        CommandWithStdin::new(command_builder)
    };

    #[cfg(not(windows))]
    let command_builder = if Path::new(command).exists() {
        let mut command_builder = Command::new("sh");
        command_builder.arg(command);
        CommandWithStdin::new(command_builder)
    } else {
        let mut command_builder = Command::new("sh");
        command_builder.arg("-lc").arg(command);
        CommandWithStdin::new(command_builder)
    };

    command_builder
}

struct CommandWithStdin {
    command: Command,
}

impl CommandWithStdin {
    fn new(command: Command) -> Self {
        Self { command }
    }

    fn stdin(&mut self, cfg: std::process::Stdio) -> &mut Self {
        self.command.stdin(cfg);
        self
    }

    fn stdout(&mut self, cfg: std::process::Stdio) -> &mut Self {
        self.command.stdout(cfg);
        self
    }

    fn stderr(&mut self, cfg: std::process::Stdio) -> &mut Self {
        self.command.stderr(cfg);
        self
    }

    fn env<K, V>(&mut self, key: K, value: V) -> &mut Self
    where
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        self.command.env(key, value);
        self
    }

    #[allow(dead_code)]
    fn output_with_stdin(&mut self, stdin: &[u8]) -> std::io::Result<std::process::Output> {
        let mut child = self.command.spawn()?;
        if let Some(mut child_stdin) = child.stdin.take() {
            use std::io::Write as _;
            child_stdin.write_all(stdin)?;
        }
        child.wait_with_output()
    }

    fn output_with_stdin_timeout(
        &mut self,
        stdin: &[u8],
        timeout: Duration,
    ) -> std::io::Result<std::process::Output> {
        let mut child = self.command.spawn()?;
        if let Some(mut child_stdin) = child.stdin.take() {
            use std::io::Write as _;
            child_stdin.write_all(stdin)?;
        }

        // 使用简单超时机制
        let start = std::time::Instant::now();
        loop {
            if child.try_wait()?.is_some() {
                return child.wait_with_output();
            }
            if start.elapsed() > timeout {
                // 超时，杀死进程
                let _ = child.kill();
                return Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    format!("Command timed out after {}s", timeout.as_secs()),
                ));
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{HookRunResult, HookRunner};
    use crate::{PluginManager, PluginManagerConfig};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("plugins-hook-runner-{label}-{nanos}"))
    }

    fn write_hook_plugin(root: &Path, name: &str, pre_message: &str, post_message: &str) {
        fs::create_dir_all(root.join(".claw-plugin")).expect("manifest dir");
        fs::create_dir_all(root.join("hooks")).expect("hooks dir");
        fs::write(
            root.join("hooks").join("pre.sh"),
            format!("#!/bin/sh\nprintf '%s\\n' '{pre_message}'\n"),
        )
        .expect("write pre hook");
        fs::write(
            root.join("hooks").join("post.sh"),
            format!("#!/bin/sh\nprintf '%s\\n' '{post_message}'\n"),
        )
        .expect("write post hook");
        fs::write(
            root.join(".claw-plugin").join("plugin.json"),
            format!(
                "{{\n  \"name\": \"{name}\",\n  \"version\": \"1.0.0\",\n  \"description\": \"hook plugin\",\n  \"hooks\": {{\n    \"PreToolUse\": [\"./hooks/pre.sh\"],\n    \"PostToolUse\": [\"./hooks/post.sh\"]\n  }}\n}}"
            ),
        )
        .expect("write plugin manifest");
    }

    #[test]
    fn collects_and_runs_hooks_from_enabled_plugins() {
        let config_home = temp_dir("config");
        let first_source_root = temp_dir("source-a");
        let second_source_root = temp_dir("source-b");
        write_hook_plugin(
            &first_source_root,
            "first",
            "plugin pre one",
            "plugin post one",
        );
        write_hook_plugin(
            &second_source_root,
            "second",
            "plugin pre two",
            "plugin post two",
        );

        let mut manager = PluginManager::new(PluginManagerConfig::new(&config_home));
        manager
            .install(first_source_root.to_str().expect("utf8 path"))
            .expect("first plugin install should succeed");
        manager
            .install(second_source_root.to_str().expect("utf8 path"))
            .expect("second plugin install should succeed");
        let registry = manager.plugin_registry().expect("registry should build");

        let runner = HookRunner::from_registry(&registry).expect("plugin hooks should load");

        let pre_result = runner.run_pre_tool_use("Read", r#"{"path":"README.md"}"#);
        assert!(!pre_result.denied);
        assert_eq!(pre_result.messages, vec![
            "plugin pre one".to_string(),
            "plugin pre two".to_string(),
        ]);
        assert_eq!(pre_result.execution_logs.len(), 2);

        let post_result = runner.run_post_tool_use("Read", r#"{"path":"README.md"}"#, "ok", false);
        assert!(!post_result.denied);
        assert_eq!(post_result.messages, vec![
            "plugin post one".to_string(),
            "plugin post two".to_string(),
        ]);
        assert_eq!(post_result.execution_logs.len(), 2);

        let _ = fs::remove_dir_all(config_home);
        let _ = fs::remove_dir_all(first_source_root);
        let _ = fs::remove_dir_all(second_source_root);
    }

    #[test]
    fn pre_tool_use_denies_when_plugin_hook_exits_two() {
        let runner = HookRunner::new(crate::PluginHooks {
            pre_tool_use: vec!["printf 'blocked by plugin'; exit 2".to_string()],
            post_tool_use: Vec::new(),
        });

        let result = runner.run_pre_tool_use("Bash", r#"{"command":"pwd"}"#);

        assert!(result.is_denied());
        assert_eq!(result.messages(), &["blocked by plugin".to_string()]);
    }
}
