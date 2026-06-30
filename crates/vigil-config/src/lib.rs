use std::{
    collections::BTreeMap,
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
    pub sources: SourceSettings,
    pub investigation: InvestigationSettings,
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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SourceSettings {
    pub inventory_files: Vec<InventoryFileSource>,
    pub runbook_files: Vec<RunbookFileSource>,
    pub alertmanagers: Vec<MockableSource>,
    pub prometheus: Vec<MockableSource>,
    pub github: Vec<GithubSource>,
    pub http: Vec<MockableSource>,
    pub dns: Vec<DnsSource>,
    pub loki: Vec<MockableSource>,
    pub grafana: Vec<MockableSource>,
    pub kubernetes: Vec<KubernetesSource>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventoryFileSource {
    pub name: String,
    pub path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunbookFileSource {
    pub name: String,
    pub dir: Option<PathBuf>,
    pub paths: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MockableSource {
    pub name: String,
    pub url: Option<String>,
    pub fixture_path: Option<PathBuf>,
    pub bearer_token_env: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DnsSource {
    pub name: String,
    pub fixture_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GithubSource {
    pub name: String,
    pub api_url: Option<String>,
    pub repo: Option<String>,
    pub fixture_path: Option<PathBuf>,
    pub bearer_token_env: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KubernetesSource {
    pub name: String,
    pub url: Option<String>,
    pub namespace: Option<String>,
    pub fixture_path: Option<PathBuf>,
    pub bearer_token_env: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvestigationSettings {
    pub max_iterations: u32,
    pub max_tool_calls: u32,
    pub max_duration_secs: u64,
}

impl Default for InvestigationSettings {
    fn default() -> Self {
        Self {
            max_iterations: 2,
            max_tool_calls: 8,
            max_duration_secs: 60,
        }
    }
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
    #[serde(default)]
    sources: FileSourcesConfig,
    #[serde(default)]
    investigation: FileInvestigationConfig,
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

#[derive(Debug, Deserialize, Default)]
struct FileSourcesConfig {
    #[serde(default)]
    inventory: BTreeMap<String, FileInventorySource>,
    #[serde(default)]
    runbook: BTreeMap<String, FileRunbookSource>,
    #[serde(default)]
    alertmanager: BTreeMap<String, FileMockableSource>,
    #[serde(default)]
    prometheus: BTreeMap<String, FileMockableSource>,
    #[serde(default)]
    github: BTreeMap<String, FileGithubSource>,
    #[serde(default)]
    http: BTreeMap<String, FileMockableSource>,
    #[serde(default)]
    dns: BTreeMap<String, FileDnsSource>,
    #[serde(default)]
    loki: BTreeMap<String, FileMockableSource>,
    #[serde(default)]
    grafana: BTreeMap<String, FileMockableSource>,
    #[serde(default)]
    kubernetes: BTreeMap<String, FileKubernetesSource>,
}

#[derive(Debug, Deserialize, Default)]
struct FileInventorySource {
    path: Option<PathBuf>,
}

#[derive(Debug, Deserialize, Default)]
struct FileRunbookSource {
    dir: Option<PathBuf>,
    #[serde(default)]
    paths: Vec<PathBuf>,
}

#[derive(Debug, Deserialize, Default)]
struct FileMockableSource {
    url: Option<String>,
    fixture_path: Option<PathBuf>,
    bearer_token_env: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct FileDnsSource {
    fixture_path: Option<PathBuf>,
}

#[derive(Debug, Deserialize, Default)]
struct FileGithubSource {
    api_url: Option<String>,
    repo: Option<String>,
    fixture_path: Option<PathBuf>,
    bearer_token_env: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct FileKubernetesSource {
    url: Option<String>,
    namespace: Option<String>,
    fixture_path: Option<PathBuf>,
    bearer_token_env: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct FileInvestigationConfig {
    max_iterations: Option<u32>,
    max_tool_calls: Option<u32>,
    max_duration_secs: Option<u64>,
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
        sources: SourceSettings::default(),
        investigation: InvestigationSettings::default(),
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
    merge_sources(file_config.sources, &mut config.sources);
    if let Some(max_iterations) = file_config.investigation.max_iterations {
        config.investigation.max_iterations = max_iterations;
    }
    if let Some(max_tool_calls) = file_config.investigation.max_tool_calls {
        config.investigation.max_tool_calls = max_tool_calls;
    }
    if let Some(max_duration_secs) = file_config.investigation.max_duration_secs {
        config.investigation.max_duration_secs = max_duration_secs;
    }

    Ok(())
}

fn merge_sources(file_sources: FileSourcesConfig, sources: &mut SourceSettings) {
    sources
        .inventory_files
        .extend(
            file_sources
                .inventory
                .into_iter()
                .map(|(name, source)| InventoryFileSource {
                    name,
                    path: source.path,
                }),
        );
    sources
        .runbook_files
        .extend(
            file_sources
                .runbook
                .into_iter()
                .map(|(name, source)| RunbookFileSource {
                    name,
                    dir: source.dir,
                    paths: source.paths,
                }),
        );
    sources
        .alertmanagers
        .extend(
            file_sources
                .alertmanager
                .into_iter()
                .map(|(name, source)| MockableSource {
                    name,
                    url: normalize_optional(source.url),
                    fixture_path: source.fixture_path,
                    bearer_token_env: normalize_optional(source.bearer_token_env),
                }),
        );
    sources
        .prometheus
        .extend(
            file_sources
                .prometheus
                .into_iter()
                .map(|(name, source)| MockableSource {
                    name,
                    url: normalize_optional(source.url),
                    fixture_path: source.fixture_path,
                    bearer_token_env: normalize_optional(source.bearer_token_env),
                }),
        );
    sources.github.extend(
        file_sources
            .github
            .into_iter()
            .map(|(name, source)| GithubSource {
                name,
                api_url: normalize_optional(source.api_url),
                repo: normalize_optional(source.repo),
                fixture_path: source.fixture_path,
                bearer_token_env: normalize_optional(source.bearer_token_env),
            }),
    );
    sources.http.extend(
        file_sources
            .http
            .into_iter()
            .map(|(name, source)| MockableSource {
                name,
                url: normalize_optional(source.url),
                fixture_path: source.fixture_path,
                bearer_token_env: normalize_optional(source.bearer_token_env),
            }),
    );
    sources.dns.extend(
        file_sources
            .dns
            .into_iter()
            .map(|(name, source)| DnsSource {
                name,
                fixture_path: source.fixture_path,
            }),
    );
    sources.loki.extend(
        file_sources
            .loki
            .into_iter()
            .map(|(name, source)| MockableSource {
                name,
                url: normalize_optional(source.url),
                fixture_path: source.fixture_path,
                bearer_token_env: normalize_optional(source.bearer_token_env),
            }),
    );
    sources.grafana.extend(
        file_sources
            .grafana
            .into_iter()
            .map(|(name, source)| MockableSource {
                name,
                url: normalize_optional(source.url),
                fixture_path: source.fixture_path,
                bearer_token_env: normalize_optional(source.bearer_token_env),
            }),
    );
    sources
        .kubernetes
        .extend(
            file_sources
                .kubernetes
                .into_iter()
                .map(|(name, source)| KubernetesSource {
                    name,
                    url: normalize_optional(source.url),
                    namespace: normalize_optional(source.namespace),
                    fixture_path: source.fixture_path,
                    bearer_token_env: normalize_optional(source.bearer_token_env),
                }),
        );
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

[investigation]
max_iterations = 1
max_tool_calls = 4
max_duration_secs = 30

[sources.inventory.local]
path = "examples/minimal/inventory.yaml"

[sources.runbook.local]
dir = "examples/minimal/runbooks"

[sources.prometheus.prod]
url = "https://prometheus.example.com"
fixture_path = "fixtures/prometheus.yaml"
bearer_token_env = "PROM_TOKEN"

[sources.github.main]
api_url = "https://api.github.example.com"
repo = "example/web"
fixture_path = "fixtures/github.yaml"

[sources.http.web]
url = "https://web.example.com/health"

[sources.dns.web]

[sources.loki.prod]
url = "https://loki.example.com"

[sources.grafana.prod]
url = "https://grafana.example.com"

[sources.kubernetes.prod]
url = "https://kubernetes.example.com"
namespace = "default"
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
        assert_eq!(config.investigation.max_iterations, 1);
        assert_eq!(config.sources.inventory_files.len(), 1);
        assert_eq!(config.sources.runbook_files.len(), 1);
        assert_eq!(config.sources.prometheus[0].name, "prod");
        assert_eq!(
            config.sources.prometheus[0].bearer_token_env.as_deref(),
            Some("PROM_TOKEN")
        );
        assert_eq!(
            config.sources.github[0].api_url.as_deref(),
            Some("https://api.github.example.com")
        );
        assert_eq!(
            config.sources.github[0].repo.as_deref(),
            Some("example/web")
        );
        assert_eq!(config.sources.http[0].name, "web");
        assert_eq!(config.sources.dns[0].name, "web");
        assert_eq!(config.sources.loki[0].name, "prod");
        assert_eq!(config.sources.grafana[0].name, "prod");
        assert_eq!(
            config.sources.kubernetes[0].namespace.as_deref(),
            Some("default")
        );
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
