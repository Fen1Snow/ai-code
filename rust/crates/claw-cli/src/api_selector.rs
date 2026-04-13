//! API Provider Selector
//!
//! Supports multiple API backends with interactive selection:
//! 1. Custom Anthropic-API (default: Zhipu GLM-4.7)
//! 2. Custom OpenAI-API
//! 3. Ollama (any_to_ollama)

use std::env;
use std::io::{self, BufRead, Write};

/// Enable ANSI escape sequences on Windows using crossterm
fn enable_ansi_support() {
    #[cfg(windows)]
    {
        let _ = crossterm::execute!(
            std::io::stdout(),
            crossterm::terminal::EnableLineWrap
        );
    }
}

/// API provider configuration
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApiProvider {
    /// Anthropic-compatible API (default: Zhipu GLM)
    Anthropic {
        base_url: String,
        auth_token: String,
        model: String,
    },
    /// OpenAI-compatible API
    OpenAi {
        base_url: String,
        api_key: String,
        model: String,
    },
    /// Ollama local API
    Ollama {
        base_url: String,
        model: String,
    },
}

impl ApiProvider {
    #[allow(dead_code)]
    pub fn model(&self) -> &str {
        match self {
            Self::Anthropic { model, .. } => model,
            Self::OpenAi { model, .. } => model,
            Self::Ollama { model, .. } => model,
        }
    }

    #[allow(dead_code)]
    pub fn base_url(&self) -> &str {
        match self {
            Self::Anthropic { base_url, .. } => base_url,
            Self::OpenAi { base_url, .. } => base_url,
            Self::Ollama { base_url, .. } => base_url,
        }
    }

    #[allow(dead_code)]
    pub fn auth(&self) -> Option<&str> {
        match self {
            Self::Anthropic { auth_token, .. } => Some(auth_token),
            Self::OpenAi { api_key, .. } => Some(api_key),
            Self::Ollama { .. } => None,
        }
    }

    pub fn apply_to_env(&self) {
        match self {
            Self::Anthropic { base_url, auth_token, model } => {
                env::set_var("ANTHROPIC_BASE_URL", base_url);
                env::set_var("ANTHROPIC_AUTH_TOKEN", auth_token);
                env::set_var("ANTHROPIC_MODEL", model);
                env::set_var("ANTHROPIC_API_KEY", auth_token);
            }
            Self::OpenAi { base_url, api_key, model } => {
                env::set_var("OPENAI_BASE_URL", base_url);
                env::set_var("OPENAI_API_KEY", api_key);
                env::set_var("OPENAI_MODEL", model);
            }
            Self::Ollama { base_url, model } => {
                env::set_var("OPENAI_BASE_URL", base_url);
                env::set_var("OLLAMA_MODEL", model);
            }
        }
    }
}

pub mod defaults {
    pub const ZHIPU_ANTHROPIC_BASE_URL: &str = "https://open.bigmodel.cn/api/anthropic";
    pub const ZHIPU_DEFAULT_MODEL: &str = "GLM-4.7";
    pub const OPENAI_DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";
    pub const OPENAI_DEFAULT_MODEL: &str = "gpt-4o";
    pub const OLLAMA_DEFAULT_BASE_URL: &str = "http://localhost:11434/v1";
    pub const OLLAMA_DEFAULT_MODEL: &str = "llama3.2";
}

pub fn has_existing_config() -> bool {
    // Check for Anthropic/Claw credentials
    if env::var("ANTHROPIC_API_KEY").is_ok() || env::var("ANTHROPIC_AUTH_TOKEN").is_ok() {
        return true;
    }
    // Check for OpenAI credentials with model set (indicates configured for OpenAI)
    if env::var("OPENAI_API_KEY").is_ok() && env::var("OPENAI_MODEL").is_ok() {
        return true;
    }
    // Check for xAI credentials
    if env::var("XAI_API_KEY").is_ok() {
        return true;
    }
    false
}

#[allow(dead_code)]
pub fn detect_from_env() -> Option<ApiProvider> {
    if let Ok(auth_token) = env::var("ANTHROPIC_AUTH_TOKEN") {
        if !auth_token.is_empty() {
            let base_url = env::var("ANTHROPIC_BASE_URL")
                .unwrap_or_else(|_| "https://api.anthropic.com".to_string());
            let model = env::var("ANTHROPIC_MODEL").unwrap_or_else(|_| "claude-opus-4-6".to_string());
            return Some(ApiProvider::Anthropic { base_url, auth_token, model });
        }
    }
    if let Ok(api_key) = env::var("OPENAI_API_KEY") {
        if !api_key.is_empty() {
            let base_url = env::var("OPENAI_BASE_URL")
                .unwrap_or_else(|_| defaults::OPENAI_DEFAULT_BASE_URL.to_string());
            let model = env::var("OPENAI_MODEL").unwrap_or_else(|_| defaults::OPENAI_DEFAULT_MODEL.to_string());
            return Some(ApiProvider::OpenAi { base_url, api_key, model });
        }
    }
    if env::var("OLLAMA_HOST").is_ok() || env::var("OLLAMA_MODEL").is_ok() {
        let base_url = env::var("OLLAMA_HOST")
            .map(|host| format!("{}/v1", host.trim_end_matches('/')))
            .unwrap_or_else(|_| defaults::OLLAMA_DEFAULT_BASE_URL.to_string());
        let model = env::var("OLLAMA_MODEL").unwrap_or_else(|_| defaults::OLLAMA_DEFAULT_MODEL.to_string());
        return Some(ApiProvider::Ollama { base_url, model });
    }
    None
}

fn read_line(prompt: &str) -> Option<String> {
    print!("{prompt}");
    let _ = io::stdout().flush();
    let stdin = io::stdin();
    let mut line = String::new();
    stdin.lock().read_line(&mut line).ok()?;
    let trimmed = line.trim();
    if trimmed.is_empty() { None } else { Some(trimmed.to_string()) }
}

fn read_line_default(prompt: &str, default: &str) -> String {
    read_line(&format!("{prompt} [{}]: ", default)).unwrap_or_else(|| default.to_string())
}

pub fn select_provider_interactive() -> Option<ApiProvider> {
    // Enable ANSI support on Windows
    enable_ansi_support();
    
    println!();
    println!("\x1b[1;36m╔════════════════════════════════════════════════════════════╗\x1b[0m");
    println!("\x1b[1;36m║\x1b[0m           \x1b[1;33m🦞 Claw Code API Configuration\x1b[0m                  \x1b[1;36m║\x1b[0m");
    println!("\x1b[1;36m╠════════════════════════════════════════════════════════════╣\x1b[0m");
    println!("\x1b[1;36m║\x1b[0m  \x1b[1;32m1.\x1b[0m Custom Anthropic-API (Zhipu GLM-4.7)             \x1b[1;36m║\x1b[0m");
    println!("\x1b[1;36m║\x1b[0m  \x1b[1;32m2.\x1b[0m Custom OpenAI-API                                 \x1b[1;36m║\x1b[0m");
    println!("\x1b[1;36m║\x1b[0m  \x1b[1;32m3.\x1b[0m Ollama (local)                                    \x1b[1;36m║\x1b[0m");
    println!("\x1b[1;36m║\x1b[0m  \x1b[1;32m4.\x1b[0m Skip (use environment)                            \x1b[1;36m║\x1b[0m");
    println!("\x1b[1;36m╚════════════════════════════════════════════════════════════╝\x1b[0m");
    println!();

    let choice = read_line("Select API provider (1-4): ")?;
    match choice.as_str() {
        "1" => configure_anthropic(),
        "2" => configure_openai(),
        "3" => configure_ollama(),
        "4" => None,
        _ => {
            eprintln!("\x1b[1;31mInvalid choice: {}\x1b[0m", choice);
            None
        }
    }
}

fn configure_anthropic() -> Option<ApiProvider> {
    println!();
    println!("\x1b[1;34m▸ Configuring Anthropic-compatible API\x1b[0m");
    println!("  \x1b[2mDefault: Zhipu GLM-4.7 via Anthropic endpoint\x1b[0m");
    println!();

    let base_url = read_line_default("Base URL", defaults::ZHIPU_ANTHROPIC_BASE_URL);
    let auth_token = read_line("Auth Token (API Key): ")?;
    let model = read_line_default("Model", defaults::ZHIPU_DEFAULT_MODEL);

    println!();
    println!("\x1b[1;32m✓ Configured Anthropic API\x1b[0m");
    println!("  Base URL: {}", base_url);
    println!("  Model:    {}", model);
    println!();

    Some(ApiProvider::Anthropic { base_url, auth_token, model })
}

fn configure_openai() -> Option<ApiProvider> {
    println!();
    println!("\x1b[1;34m▸ Configuring OpenAI-compatible API\x1b[0m");
    println!();

    let base_url = read_line_default("Base URL", defaults::OPENAI_DEFAULT_BASE_URL);
    let api_key = read_line("API Key: ")?;
    let model = read_line_default("Model", defaults::OPENAI_DEFAULT_MODEL);

    println!();
    println!("\x1b[1;32m✓ Configured OpenAI API\x1b[0m");
    println!("  Base URL: {}", base_url);
    println!("  Model:    {}", model);
    println!();

    Some(ApiProvider::OpenAi { base_url, api_key, model })
}

fn configure_ollama() -> Option<ApiProvider> {
    println!();
    println!("\x1b[1;34m▸ Configuring Ollama (local)\x1b[0m");
    println!("  \x1b[2mNo API key required\x1b[0m");
    println!();

    let base_url = read_line_default("Base URL", defaults::OLLAMA_DEFAULT_BASE_URL);
    let model = read_line_default("Model", defaults::OLLAMA_DEFAULT_MODEL);

    println!();
    println!("\x1b[1;32m✓ Configured Ollama\x1b[0m");
    println!("  Base URL: {}", base_url);
    println!("  Model:    {}", model);
    println!();

    Some(ApiProvider::Ollama { base_url, model })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_model() {
        let provider = ApiProvider::Anthropic {
            base_url: "https://api.example.com".to_string(),
            auth_token: "token".to_string(),
            model: "GLM-4.7".to_string(),
        };
        assert_eq!(provider.model(), "GLM-4.7");
        assert_eq!(provider.base_url(), "https://api.example.com");
        assert_eq!(provider.auth(), Some("token"));
    }

    #[test]
    fn test_ollama_no_auth() {
        let provider = ApiProvider::Ollama {
            base_url: "http://localhost:11434/v1".to_string(),
            model: "llama3.2".to_string(),
        };
        assert_eq!(provider.auth(), None);
    }

    #[test]
    fn test_defaults() {
        assert!(defaults::ZHIPU_ANTHROPIC_BASE_URL.contains("bigmodel.cn"));
        assert_eq!(defaults::ZHIPU_DEFAULT_MODEL, "GLM-4.7");
    }
}
