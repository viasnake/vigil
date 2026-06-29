use std::{
    env, fs,
    path::{Path, PathBuf},
};

use serde::Deserialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("configuration file '{path}' could not be read: {source}")]
    ReadFile {
        path: String,
        source: std::io::Error,
    },
    #[error("configuration file '{path}' is not valid TOML: {source}")]
    ParseToml {
        path: String,
        source: toml::de::Error,
    },
    #[error("missing {setting}. {hint}")]
    MissingSetting {
        setting: &'static str,
        hint: &'static str,
    },
    #[error("request timeout must be greater than zero")]
    InvalidTimeout,
    #[error("retry count must be 5 or lower")]
    InvalidRetryCount,
    #[error("input file '{path}' is not readable: {source}")]
    UnreadableInput {
        path: String,
        source: std::io::Error,
    },
    #[error("output path '{path}' parent directory does not exist")]
    MissingOutputParent { path: String },
    #[error("output path '{path}' parent is not a directory")]
    OutputParentNotDirectory { path: String },
}

#[derive(Debug, Clone, Default)]
pub struct ConfigOverrides {
    pub account_id: Option<String>,
    pub api_token: Option<String>,
    pub gateway_id: Option<String>,
    pub model: Option<String>,
    pub endpoint: Option<CloudflareEndpoint>,
    pub request_timeout_secs: Option<u64>,
    pub retry_count: Option<u32>,
    pub output_format: Option<OutputFormat>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum OutputFormat {
    #[default]
    Markdown,
    Json,
}

impl OutputFormat {
    fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "markdown" | "md" => Some(Self::Markdown),
            "json" => Some(Self::Json),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum CloudflareEndpoint {
    #[default]
    Rest,
    Gateway,
}

impl CloudflareEndpoint {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "rest" => Some(Self::Rest),
            "gateway" | "provider-native" | "provider_native" => Some(Self::Gateway),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Rest => "rest",
            Self::Gateway => "gateway",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedConfig {
    pub cloudflare: CloudflareSettings,
    pub output_format: OutputFormat,
}

#[derive(Debug, Clone)]
pub struct CloudflareSettings {
    pub account_id: Option<String>,
    pub api_token: Option<String>,
    pub gateway_id: Option<String>,
    pub model: String,
    pub endpoint: CloudflareEndpoint,
    pub request_timeout_secs: u64,
    pub retry_count: u32,
}

impl CloudflareSettings {
    pub fn validate_for_llm(&self) -> Result<(), ConfigError> {
        require_setting(
            self.account_id.as_deref(),
            "Cloudflare account ID",
            "Set CLOUDFLARE_ACCOUNT_ID, add cloudflare.account_id to the config file, or pass --account-id.",
        )?;
        require_setting(
            self.api_token.as_deref(),
            "Cloudflare API token",
            "Set CLOUDFLARE_API_TOKEN, add cloudflare.api_token to the config file, or pass --api-token.",
        )?;
        require_setting(
            self.gateway_id.as_deref(),
            "Cloudflare AI Gateway ID",
            "Set VIGIL_CLOUDFLARE_GATEWAY_ID, add cloudflare.gateway_id to the config file, or pass --gateway-id.",
        )?;
        if self.model.trim().is_empty() {
            return Err(ConfigError::MissingSetting {
                setting: "LLM model",
                hint:
                    "Set VIGIL_LLM_MODEL, add cloudflare.model to the config file, or pass --model.",
            });
        }
        if self.request_timeout_secs == 0 {
            return Err(ConfigError::InvalidTimeout);
        }
        if self.retry_count > 5 {
            return Err(ConfigError::InvalidRetryCount);
        }
        Ok(())
    }

    pub fn redacted_summary(&self) -> String {
        let account = self.account_id.as_deref().unwrap_or("<missing>");
        let gateway = self.gateway_id.as_deref().unwrap_or("<missing>");
        let token = if self
            .api_token
            .as_deref()
            .is_some_and(|value| !value.is_empty())
        {
            "<set>"
        } else {
            "<missing>"
        };

        format!(
            "account_id={account}, gateway_id={gateway}, api_token={token}, model={}, endpoint={}, timeout={}s, retries={}",
            self.model,
            self.endpoint.as_str(),
            self.request_timeout_secs,
            self.retry_count
        )
    }
}

#[derive(Debug, Deserialize, Default)]
struct FileConfig {
    #[serde(default)]
    cloudflare: FileCloudflareConfig,
    #[serde(default)]
    output: FileOutputConfig,
}

#[derive(Debug, Deserialize, Default)]
struct FileCloudflareConfig {
    account_id: Option<String>,
    api_token: Option<String>,
    gateway_id: Option<String>,
    model: Option<String>,
    endpoint: Option<String>,
    request_timeout_secs: Option<u64>,
    retry_count: Option<u32>,
}

#[derive(Debug, Deserialize, Default)]
struct FileOutputConfig {
    format: Option<String>,
}

pub fn load_config(
    config_path: Option<&Path>,
    overrides: ConfigOverrides,
) -> Result<ResolvedConfig, ConfigError> {
    let mut config = ResolvedConfig {
        cloudflare: CloudflareSettings {
            account_id: None,
            api_token: None,
            gateway_id: None,
            model: "openai/gpt-4.1".to_string(),
            endpoint: CloudflareEndpoint::Rest,
            request_timeout_secs: 30,
            retry_count: 1,
        },
        output_format: OutputFormat::Markdown,
    };

    if let Some(path) = config_path {
        merge_file_config(path, &mut config)?;
    }

    merge_env_config(&mut config);
    merge_overrides(&mut config, overrides);

    if config.cloudflare.request_timeout_secs == 0 {
        return Err(ConfigError::InvalidTimeout);
    }
    if config.cloudflare.retry_count > 5 {
        return Err(ConfigError::InvalidRetryCount);
    }

    Ok(config)
}

pub fn check_readable_file(path: &Path) -> Result<(), ConfigError> {
    fs::File::open(path)
        .map(|_| ())
        .map_err(|source| ConfigError::UnreadableInput {
            path: path.display().to_string(),
            source,
        })
}

pub fn check_output_path(path: &Path) -> Result<(), ConfigError> {
    let parent = match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => return Ok(()),
    };

    if !parent.exists() {
        return Err(ConfigError::MissingOutputParent {
            path: path.display().to_string(),
        });
    }
    if !parent.is_dir() {
        return Err(ConfigError::OutputParentNotDirectory {
            path: path.display().to_string(),
        });
    }

    Ok(())
}

fn merge_file_config(path: &Path, config: &mut ResolvedConfig) -> Result<(), ConfigError> {
    let text = fs::read_to_string(path).map_err(|source| ConfigError::ReadFile {
        path: path.display().to_string(),
        source,
    })?;
    let file_config: FileConfig =
        toml::from_str(&text).map_err(|source| ConfigError::ParseToml {
            path: path.display().to_string(),
            source,
        })?;

    assign_if_some(
        &mut config.cloudflare.account_id,
        normalize_optional(file_config.cloudflare.account_id),
    );
    assign_if_some(
        &mut config.cloudflare.api_token,
        normalize_optional(file_config.cloudflare.api_token),
    );
    assign_if_some(
        &mut config.cloudflare.gateway_id,
        normalize_optional(file_config.cloudflare.gateway_id),
    );
    if let Some(model) = normalize_optional(file_config.cloudflare.model) {
        config.cloudflare.model = model;
    }
    if let Some(endpoint) = file_config
        .cloudflare
        .endpoint
        .as_deref()
        .and_then(CloudflareEndpoint::parse)
    {
        config.cloudflare.endpoint = endpoint;
    }
    if let Some(timeout) = file_config.cloudflare.request_timeout_secs {
        config.cloudflare.request_timeout_secs = timeout;
    }
    if let Some(retry_count) = file_config.cloudflare.retry_count {
        config.cloudflare.retry_count = retry_count;
    }
    if let Some(format) = file_config
        .output
        .format
        .as_deref()
        .and_then(OutputFormat::parse)
    {
        config.output_format = format;
    }

    Ok(())
}

fn merge_env_config(config: &mut ResolvedConfig) {
    assign_if_some(
        &mut config.cloudflare.account_id,
        env_string("CLOUDFLARE_ACCOUNT_ID"),
    );
    assign_if_some(
        &mut config.cloudflare.api_token,
        env_string("CLOUDFLARE_API_TOKEN"),
    );
    assign_if_some(
        &mut config.cloudflare.gateway_id,
        env_string("VIGIL_CLOUDFLARE_GATEWAY_ID"),
    );
    if let Some(model) = env_string("VIGIL_LLM_MODEL") {
        config.cloudflare.model = model;
    }
    if let Some(endpoint) = env_string("VIGIL_CLOUDFLARE_ENDPOINT")
        .as_deref()
        .and_then(CloudflareEndpoint::parse)
    {
        config.cloudflare.endpoint = endpoint;
    }
}

fn merge_overrides(config: &mut ResolvedConfig, overrides: ConfigOverrides) {
    assign_if_some(
        &mut config.cloudflare.account_id,
        normalize_optional(overrides.account_id),
    );
    assign_if_some(
        &mut config.cloudflare.api_token,
        normalize_optional(overrides.api_token),
    );
    assign_if_some(
        &mut config.cloudflare.gateway_id,
        normalize_optional(overrides.gateway_id),
    );
    if let Some(model) = normalize_optional(overrides.model) {
        config.cloudflare.model = model;
    }
    if let Some(endpoint) = overrides.endpoint {
        config.cloudflare.endpoint = endpoint;
    }
    if let Some(timeout) = overrides.request_timeout_secs {
        config.cloudflare.request_timeout_secs = timeout;
    }
    if let Some(retry_count) = overrides.retry_count {
        config.cloudflare.retry_count = retry_count;
    }
    if let Some(output_format) = overrides.output_format {
        config.output_format = output_format;
    }
}

fn require_setting(
    value: Option<&str>,
    setting: &'static str,
    hint: &'static str,
) -> Result<(), ConfigError> {
    match value {
        Some(value) if !value.trim().is_empty() => Ok(()),
        _ => Err(ConfigError::MissingSetting { setting, hint }),
    }
}

fn assign_if_some(target: &mut Option<String>, value: Option<String>) {
    if let Some(value) = value {
        *target = Some(value);
    }
}

fn normalize_optional(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn env_string(name: &str) -> Option<String> {
    normalize_optional(env::var(name).ok())
}

pub fn path_list(paths: &[PathBuf]) -> Vec<String> {
    paths
        .iter()
        .map(|path| path.display().to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use tempfile::NamedTempFile;

    use super::*;

    #[test]
    fn loads_config_file_and_applies_overrides() -> Result<(), Box<dyn std::error::Error>> {
        let mut file = NamedTempFile::new()?;
        writeln!(
            file,
            r#"
[cloudflare]
account_id = "from-file"
api_token = "..."
gateway_id = "file-gateway"
model = "openai/gpt-4.1"
endpoint = "gateway"
request_timeout_secs = 12
retry_count = 2

[output]
format = "json"
"#
        )?;

        let config = load_config(
            Some(file.path()),
            ConfigOverrides {
                gateway_id: Some("cli-gateway".to_string()),
                model: Some("openai/gpt-4.1-mini".to_string()),
                ..ConfigOverrides::default()
            },
        )?;

        assert_eq!(config.cloudflare.account_id.as_deref(), Some("from-file"));
        assert_eq!(config.cloudflare.gateway_id.as_deref(), Some("cli-gateway"));
        assert_eq!(config.cloudflare.model, "openai/gpt-4.1-mini");
        assert_eq!(config.cloudflare.endpoint, CloudflareEndpoint::Gateway);
        assert_eq!(config.output_format, OutputFormat::Json);
        Ok(())
    }

    #[test]
    fn validates_missing_cloudflare_token() {
        let config = CloudflareSettings {
            account_id: Some("account".to_string()),
            api_token: None,
            gateway_id: Some("gateway".to_string()),
            model: "openai/gpt-4.1".to_string(),
            endpoint: CloudflareEndpoint::Rest,
            request_timeout_secs: 30,
            retry_count: 0,
        };

        let error = config.validate_for_llm().err();
        assert!(matches!(
            error,
            Some(ConfigError::MissingSetting {
                setting: "Cloudflare API token",
                ..
            })
        ));
    }

    #[test]
    fn redacted_summary_does_not_expose_token() {
        let config = CloudflareSettings {
            account_id: Some("account".to_string()),
            api_token: Some("secret-token-value".to_string()),
            gateway_id: Some("gateway".to_string()),
            model: "openai/gpt-4.1".to_string(),
            endpoint: CloudflareEndpoint::Rest,
            request_timeout_secs: 30,
            retry_count: 1,
        };

        let summary = config.redacted_summary();
        assert!(summary.contains("api_token=<set>"));
        assert!(!summary.contains("secret-token-value"));
    }

    #[test]
    fn parses_cloudflare_endpoint_values() {
        assert_eq!(
            CloudflareEndpoint::parse("gateway"),
            Some(CloudflareEndpoint::Gateway)
        );
        assert_eq!(
            CloudflareEndpoint::parse("provider-native"),
            Some(CloudflareEndpoint::Gateway)
        );
        assert_eq!(
            CloudflareEndpoint::parse("rest"),
            Some(CloudflareEndpoint::Rest)
        );
        assert_eq!(CloudflareEndpoint::parse("unknown"), None);
    }
}
