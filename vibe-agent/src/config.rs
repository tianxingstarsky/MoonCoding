use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// API 来源：自定义 API 或公司托管 API
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ApiSource {
    #[serde(rename = "custom")]
    Custom,
    #[serde(rename = "managed")]
    Managed,
}

impl Default for ApiSource {
    fn default() -> Self {
        ApiSource::Custom
    }
}

/// 托管 API 配置 —— 未来由公司服务器统一管理用量
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ManagedApiConfig {
    /// 公司 API 网关地址
    pub endpoint: String,
    /// 认证令牌
    pub auth_token: String,
    /// 项目标识，用于用量统计
    #[serde(default)]
    pub project_id: Option<String>,
}

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

fn default_max_tokens() -> u64 {
    32768
}
fn default_temperature() -> f64 {
    0.1
}

/// mooncoding 配置文件格式
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MoonCodingToml {
    #[serde(default)]
    pub api_source: Option<String>,
    #[serde(default)]
    pub managed_api: Option<ManagedApiConfig>,
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
    /// `desktop` (default) or `board` — drives prompt runtime facts.
    #[serde(default)]
    pub deployment_target: Option<String>,
}

/// Where the full Qt6 GUI is expected to run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeploymentTarget {
    Desktop,
    Board,
}

impl DeploymentTarget {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Desktop => "desktop",
            Self::Board => "board",
        }
    }

    pub fn parse(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "board" | "lyra" | "linuxfb" | "embedded" => Self::Board,
            _ => Self::Desktop,
        }
    }
}

/// 运行时配置 —— 从 .mooncoding.toml + 环境变量 + CLI flags 合并
#[derive(Debug, Clone)]
pub struct Config {
    pub language: String,
    pub api_source: ApiSource,
    pub managed_api: Option<ManagedApiConfig>,
    pub provider: ProviderConfig,
    pub agent: AgentToml,
    pub workspace: PathBuf,
    pub vibe_exe: PathBuf,
    pub session_dir: PathBuf,
    pub deployment_target: DeploymentTarget,
}

impl Config {
    /// 三层合并: env(最高) -> toml -> 内置默认
    pub fn load(root: &Path) -> Result<Self> {
        let toml = Self::load_toml(root);
        let provider = Self::build_provider(&toml);
        let agent = toml.agent;
        let language = "zh".to_string();
        let api_source = match toml.api_source.as_deref() {
            Some("managed") => ApiSource::Managed,
            _ => ApiSource::Custom,
        };
        let managed_api = toml.managed_api;
        let vibe_exe = Self::find_vibe_exe();
        let session_dir = root.join(".mooncoding").join("sessions");
        let deployment_target = Self::resolve_deployment_target(&agent);
        Ok(Self {
            language,
            api_source,
            managed_api,
            provider,
            agent,
            workspace: root.to_path_buf(),
            vibe_exe,
            session_dir,
            deployment_target,
        })
    }

    fn resolve_deployment_target(agent: &AgentToml) -> DeploymentTarget {
        if let Ok(env) = std::env::var("MOONCODING_DEPLOYMENT_TARGET") {
            if !env.is_empty() {
                return DeploymentTarget::parse(&env);
            }
        }
        agent
            .deployment_target
            .as_deref()
            .map(DeploymentTarget::parse)
            .unwrap_or(DeploymentTarget::Desktop)
    }

    fn load_toml(root: &Path) -> MoonCodingToml {
        let paths = [
            root.join(".mooncoding.toml"),
            dirs_home()
                .join(".config")
                .join("mooncoding")
                .join("config.toml"),
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
            pt.and_then(|p| p.base_url.as_deref())
                .unwrap_or("https://api.deepseek.com"),
        );
        let model = env_or(
            "MOONCODING_MODEL",
            pt.and_then(|p| p.model.as_deref())
                .unwrap_or("deepseek-v4-flash"),
        );
        let api_key = env_or(
            "MOONCODING_API_KEY",
            pt.and_then(|p| p.api_key.as_deref()).unwrap_or(""),
        );
        // 向后兼容: DEEPSEEK_API_KEY / OPENAI_API_KEY
        let api_key = if api_key.is_empty() {
            env_or("DEEPSEEK_API_KEY", "")
        } else {
            api_key
        };
        let api_key = if api_key.is_empty() {
            env_or("OPENAI_API_KEY", "")
        } else {
            api_key
        };

        let max_tokens = env_u64(
            "MOONCODING_MAX_TOKENS",
            pt.and_then(|p| p.max_tokens).unwrap_or(4096),
        );
        let temperature = env_f64(
            "MOONCODING_TEMPERATURE",
            pt.and_then(|p| p.temperature).unwrap_or(0.1),
        );
        ProviderConfig {
            base_url,
            model,
            api_key,
            max_tokens,
            temperature,
        }
    }

    fn find_vibe_exe() -> PathBuf {
        if let Ok(p) = std::env::var("VIBE_PATH") {
            let path = PathBuf::from(p);
            if path.is_file() {
                return path;
            }
        }

        let mut candidates = Vec::new();
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                #[cfg(windows)]
                candidates.push(dir.join("vibe.exe"));
                #[cfg(not(windows))]
                candidates.push(dir.join("vibe"));
            }
        }
        #[cfg(windows)]
        {
            candidates.push(PathBuf::from("vibe/target/release/vibe.exe"));
            candidates.push(PathBuf::from("../vibe/target/release/vibe.exe"));
            candidates.push(PathBuf::from("build/vibe-target/release/vibe.exe"));
            candidates.push(PathBuf::from("build/vibe-ui/vibe.exe"));
        }
        #[cfg(not(windows))]
        {
            candidates.push(PathBuf::from("vibe/target/release/vibe"));
            candidates.push(PathBuf::from("../vibe/target/release/vibe"));
            candidates.push(PathBuf::from("build/vibe-target/release/vibe"));
            candidates.push(PathBuf::from("build/vibe-ui/vibe"));
        }

        for candidate in candidates {
            if let Ok(canonical) = candidate.canonicalize() {
                if canonical.is_file() {
                    return canonical;
                }
            }
        }
        PathBuf::from("vibe")
    }
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key)
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| default.to_string())
}

fn env_u64(key: &str, default: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(default)
}

fn env_f64(key: &str, default: f64) -> f64 {
    std::env::var(key)
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(default)
}

fn dirs_home() -> PathBuf {
    std::env::var("USERPROFILE")
        .map(PathBuf::from)
        .or_else(|_| std::env::var("HOME").map(PathBuf::from))
        .unwrap_or_else(|_| PathBuf::from("."))
}
