use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Provider 配置: OpenAI-compatible 端点
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub base_url: String,
    pub model: String,
    pub api_key: String,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u64,
    #[serde(default = "default_temperature")]
    pub temperature: f64,
}

fn default_max_tokens() -> u64 { 4096 }
fn default_temperature() -> f64 { 0.1 }

/// mooncoding 配置文件格式
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MoonCodingToml {
    #[serde(default)]
    pub provider: Option<ProviderToml>,
    #[serde(default)]
    pub agent: AgentToml,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderToml {
    pub base_url: Option<String>,
    pub model: Option<String>,
    pub api_key: Option<String>,
    pub max_tokens: Option<u64>,
    pub temperature: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AgentToml {
    #[serde(default)]
    pub max_steps: Option<u64>,
    #[serde(default)]
    pub prune_after: Option<usize>,
    #[serde(default)]
    pub prune_keep: Option<usize>,
    #[serde(default)]
    pub tree_enabled: bool,
}

/// 运行时配置 —— 从 .mooncoding.toml + 环境变量 + CLI flags 合并
#[derive(Debug, Clone)]
pub struct Config {
    pub provider: ProviderConfig,
    pub agent: AgentToml,
    pub vibe_exe: PathBuf,
    pub session_dir: PathBuf,
}

impl Config {
    /// 三层合并: env(最高) -> toml -> 内置默认
    pub fn load(root: &Path) -> Result<Self> {
        let toml = Self::load_toml(root);
        let provider = Self::build_provider(&toml);
        let agent = toml.agent;
        let vibe_exe = Self::find_vibe_exe();
        let session_dir = root.join(".mooncoding").join("sessions");
        Ok(Self { provider, agent, vibe_exe, session_dir })
    }

    fn load_toml(root: &Path) -> MoonCodingToml {
        let paths = [
            root.join(".mooncoding.toml"),
            dirs_home().join(".config").join("mooncoding").join("config.toml"),
        ];
        for p in &paths {
            if let Ok(content) = std::fs::read_to_string(p) {
                if let Ok(t) = toml::from_str::<MoonCodingToml>(&content) {
                    return t;
                }
            }
        }
        MoonCodingToml::default()
    }

    fn build_provider(toml: &MoonCodingToml) -> ProviderConfig {
        let pt = toml.provider.as_ref();
        let base_url = env_or(
            "MOONCODING_BASE_URL",
            pt.and_then(|p| p.base_url.as_deref()).unwrap_or("https://api.deepseek.com"),
        );
        let model = env_or(
            "MOONCODING_MODEL",
            pt.and_then(|p| p.model.as_deref()).unwrap_or("deepseek-v4-flash"),
        );
        let api_key = env_or(
            "MOONCODING_API_KEY",
            pt.and_then(|p| p.api_key.as_deref()).unwrap_or(""),
        );
        // 向后兼容: DEEPSEEK_API_KEY / OPENAI_API_KEY
        let api_key = if api_key.is_empty() {
            env_or("DEEPSEEK_API_KEY", "")
        } else { api_key };
        let api_key = if api_key.is_empty() {
            env_or("OPENAI_API_KEY", "")
        } else { api_key };

        let max_tokens = env_u64("MOONCODING_MAX_TOKENS", pt.and_then(|p| p.max_tokens).unwrap_or(4096));
        let temperature = env_f64("MOONCODING_TEMPERATURE", pt.and_then(|p| p.temperature).unwrap_or(0.1));
        ProviderConfig { base_url, model, api_key, max_tokens, temperature }
    }

    fn find_vibe_exe() -> PathBuf {
        if let Ok(p) = std::env::var("VIBE_PATH") { return PathBuf::from(p); }
        let sibling = PathBuf::from("..").join("vibe").join("target").join("release");
        #[cfg(windows)] let exe = sibling.join("vibe.exe");
        #[cfg(not(windows))] let exe = sibling.join("vibe");
        if let Ok(c) = std::fs::canonicalize(&exe) { return c; }
        PathBuf::from("vibe")
    }
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).ok().filter(|s| !s.is_empty()).unwrap_or_else(|| default.to_string())
}

fn env_u64(key: &str, default: u64) -> u64 {
    std::env::var(key).ok().and_then(|s| s.parse().ok()).unwrap_or(default)
}

fn env_f64(key: &str, default: f64) -> f64 {
    std::env::var(key).ok().and_then(|s| s.parse().ok()).unwrap_or(default)
}

fn dirs_home() -> PathBuf {
    std::env::var("USERPROFILE").map(PathBuf::from)
        .or_else(|_| std::env::var("HOME").map(PathBuf::from))
        .unwrap_or_else(|_| PathBuf::from("."))
}