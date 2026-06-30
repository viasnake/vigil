use std::{
    collections::{BTreeMap, BTreeSet},
    str::FromStr,
};

use schemars::{schema_for, JsonSchema};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ModelError {
    #[error("reasoning result is not valid JSON: {0}")]
    InvalidJson(#[from] serde_json::Error),
    #[error("reasoning result schema could not be compiled: {0}")]
    SchemaCompile(String),
    #[error("reasoning result failed schema validation: {0}")]
    SchemaValidation(String),
    #[error("reasoning result failed semantic validation: {0}")]
    SemanticValidation(String),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TargetKind {
    Service,
    Host,
    Component,
    Endpoint,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct Target {
    pub id: String,
    #[serde(default)]
    pub kind: TargetKind,
    pub name: String,
    #[serde(default)]
    pub environment: Option<String>,
    #[serde(default)]
    pub service: Option<String>,
    #[serde(default)]
    pub host: Option<String>,
    #[serde(default)]
    pub labels: BTreeMap<String, String>,
    #[serde(default)]
    pub criticality: Option<String>,
    #[serde(default)]
    pub metadata: BTreeMap<String, Value>,
}

impl Target {
    pub fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();
        if self.id.trim().is_empty() {
            errors.push("target.id must not be empty".to_string());
        }
        if self.name.trim().is_empty() {
            errors.push(format!("target '{}' name must not be empty", self.id));
        }
        errors
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct Alert {
    pub id: String,
    pub name: String,
    pub severity: String,
    pub status: String,
    pub summary: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub started_at: Option<String>,
    #[serde(default)]
    pub ended_at: Option<String>,
    #[serde(default)]
    pub labels: BTreeMap<String, String>,
    #[serde(default)]
    pub annotations: BTreeMap<String, Value>,
    #[serde(default)]
    pub source: Option<String>,
}

impl Alert {
    pub fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();
        if self.id.trim().is_empty() {
            errors.push("alert.id must not be empty".to_string());
        }
        if self.name.trim().is_empty() {
            errors.push(format!("alert '{}' name must not be empty", self.id));
        }
        if self.severity.trim().is_empty() {
            errors.push(format!("alert '{}' severity must not be empty", self.id));
        }
        if self.summary.trim().is_empty() {
            errors.push(format!("alert '{}' summary must not be empty", self.id));
        }
        errors
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct CaseManifest {
    pub id: String,
    pub title: String,
    pub severity: String,
    pub status: String,
    pub target: String,
    pub summary: String,
    pub created_at: String,
}

impl CaseManifest {
    pub fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();
        if self.id.trim().is_empty() {
            errors.push("case id must not be empty".to_string());
        }
        if self.title.trim().is_empty() {
            errors.push(format!("case '{}' title must not be empty", self.id));
        }
        if self.severity.trim().is_empty() {
            errors.push(format!("case '{}' severity must not be empty", self.id));
        }
        if self.status.trim().is_empty() {
            errors.push(format!("case '{}' status must not be empty", self.id));
        }
        if self.target.trim().is_empty() {
            errors.push(format!("case '{}' target must not be empty", self.id));
        }
        if self.summary.trim().is_empty() {
            errors.push(format!("case '{}' summary must not be empty", self.id));
        }
        if self.created_at.trim().is_empty() {
            errors.push(format!("case '{}' created_at must not be empty", self.id));
        }
        errors
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Default)]
pub struct Inventory {
    #[serde(default)]
    pub targets: Vec<Target>,
    #[serde(default)]
    pub services: Vec<Service>,
    #[serde(default)]
    pub hosts: Vec<Host>,
    #[serde(default)]
    pub dependencies: Vec<Dependency>,
    #[serde(default)]
    pub labels: BTreeMap<String, String>,
    #[serde(default)]
    pub metadata: BTreeMap<String, Value>,
}

impl Inventory {
    pub fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();
        if self.targets.is_empty() && self.services.is_empty() && self.hosts.is_empty() {
            errors.push("inventory must include at least one target, service, or host".to_string());
        }
        for target in &self.targets {
            errors.extend(target.validate());
        }
        for service in &self.services {
            if service.id.trim().is_empty() {
                errors.push("service.id must not be empty".to_string());
            }
            if service.name.trim().is_empty() {
                errors.push(format!("service '{}' name must not be empty", service.id));
            }
        }
        for host in &self.hosts {
            if host.id.trim().is_empty() {
                errors.push("host.id must not be empty".to_string());
            }
            if host.name.trim().is_empty() {
                errors.push(format!("host '{}' name must not be empty", host.id));
            }
        }
        errors
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Default)]
pub struct Service {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub target_ids: Vec<String>,
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(default)]
    pub labels: BTreeMap<String, String>,
    #[serde(default)]
    pub metadata: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Default)]
pub struct Host {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub environment: Option<String>,
    #[serde(default)]
    pub labels: BTreeMap<String, String>,
    #[serde(default)]
    pub metadata: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Default)]
pub struct Dependency {
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub relation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Default)]
pub struct Runbook {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub applies_to: Vec<String>,
    #[serde(default)]
    pub symptoms: Vec<String>,
    #[serde(default)]
    pub checks: Vec<RunbookCheck>,
    #[serde(default)]
    pub notes: Vec<String>,
    #[serde(default)]
    pub references: Vec<SourceReference>,
}

impl Runbook {
    pub fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();
        if self.id.trim().is_empty() {
            errors.push("runbook.id must not be empty".to_string());
        }
        if self.title.trim().is_empty() {
            errors.push(format!("runbook '{}' title must not be empty", self.id));
        }
        for check in &self.checks {
            if check.id.trim().is_empty() {
                errors.push(format!("runbook '{}' check.id must not be empty", self.id));
            }
            if !check.read_only {
                errors.push(format!(
                    "runbook '{}' check '{}' must be read_only for Vigil 1.0",
                    self.id, check.id
                ));
            }
        }
        errors
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct RunbookCheck {
    pub id: String,
    pub title: String,
    pub description: String,
    #[serde(default = "default_true")]
    pub read_only: bool,
    #[serde(default)]
    pub references: Vec<SourceReference>,
}

impl Default for RunbookCheck {
    fn default() -> Self {
        Self {
            id: String::new(),
            title: String::new(),
            description: String::new(),
            read_only: true,
            references: Vec::new(),
        }
    }
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq, Default)]
pub struct SourceReference {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    Alert,
    Metric,
    Log,
    Change,
    Runbook,
    Inventory,
    OperatorInput,
    #[default]
    External,
}

impl EvidenceKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Alert => "alert",
            Self::Metric => "metric",
            Self::Log => "log",
            Self::Change => "change",
            Self::Runbook => "runbook",
            Self::Inventory => "inventory",
            Self::OperatorInput => "operator_input",
            Self::External => "external",
        }
    }

    pub fn file_prefix(&self) -> &'static str {
        match self {
            Self::OperatorInput => "operator-note",
            other => other.as_str(),
        }
    }
}

impl FromStr for EvidenceKind {
    type Err = ModelError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().replace('-', "_").as_str() {
            "alert" => Ok(Self::Alert),
            "metric" => Ok(Self::Metric),
            "log" => Ok(Self::Log),
            "change" => Ok(Self::Change),
            "runbook" => Ok(Self::Runbook),
            "inventory" => Ok(Self::Inventory),
            "operator_input" | "operator_note" | "observation" => Ok(Self::OperatorInput),
            "external" => Ok(Self::External),
            other => Err(ModelError::SemanticValidation(format!(
                "unsupported evidence kind '{other}'"
            ))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct Evidence {
    pub id: String,
    #[serde(default)]
    pub kind: EvidenceKind,
    pub summary: String,
    pub source: EvidenceSource,
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub timestamp: Option<String>,
    #[serde(default = "default_confidence")]
    pub confidence: f32,
    #[serde(default)]
    pub data: Value,
    #[serde(default)]
    pub references: Vec<SourceReference>,
}

impl Evidence {
    pub fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();
        if self.id.trim().is_empty() {
            errors.push("evidence.id must not be empty".to_string());
        }
        if self.summary.trim().is_empty() {
            errors.push(format!("evidence '{}' summary must not be empty", self.id));
        }
        if !(0.0..=1.0).contains(&self.confidence) {
            errors.push(format!(
                "evidence '{}' confidence must be between 0.0 and 1.0",
                self.id
            ));
        }
        errors
    }
}

fn default_confidence() -> f32 {
    1.0
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Default)]
pub struct EvidenceSource {
    pub kind: String,
    pub name: String,
    #[serde(default)]
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum SourceKind {
    InventoryFile,
    RunbookFile,
    Alertmanager,
    Prometheus,
    Github,
    Http,
    Dns,
    Loki,
    Grafana,
    Kubernetes,
}

impl SourceKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::InventoryFile => "inventory-file",
            Self::RunbookFile => "runbook-file",
            Self::Alertmanager => "alertmanager",
            Self::Prometheus => "prometheus",
            Self::Github => "github",
            Self::Http => "http",
            Self::Dns => "dns",
            Self::Loki => "loki",
            Self::Grafana => "grafana",
            Self::Kubernetes => "kubernetes",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct Source {
    pub id: String,
    pub kind: SourceKind,
    pub name: String,
    pub read_only: bool,
    #[serde(default)]
    pub config: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityKind {
    Inventory,
    Runbook,
    Alerts,
    Metrics,
    Changes,
    Http,
    Dns,
    Logs,
    Dashboards,
    Kubernetes,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct Capability {
    pub id: String,
    pub kind: CapabilityKind,
    pub source_id: String,
    pub adapter: SourceKind,
    pub read_only: bool,
    pub description: String,
    #[serde(default)]
    pub input_schema: BTreeMap<String, String>,
    pub risk: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct ToolCall {
    pub id: String,
    pub capability_id: String,
    pub source_id: String,
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub since: Option<String>,
    pub reason: String,
    #[serde(default)]
    pub inputs: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct ToolPlan {
    pub id: String,
    pub rationale: String,
    #[serde(default)]
    pub calls: Vec<ToolCall>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ToolResultStatus {
    Succeeded,
    Skipped,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct ToolResult {
    pub call_id: String,
    pub capability_id: String,
    pub source_id: String,
    pub status: ToolResultStatus,
    pub started_at: String,
    pub completed_at: String,
    #[serde(default)]
    pub evidence: Vec<Evidence>,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct InvestigationBudget {
    pub max_iterations: u32,
    pub max_tool_calls: u32,
    pub max_duration_secs: u64,
}

impl Default for InvestigationBudget {
    fn default() -> Self {
        Self {
            max_iterations: 2,
            max_tool_calls: 8,
            max_duration_secs: 60,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct InvestigationIteration {
    pub index: u32,
    pub plan: ToolPlan,
    #[serde(default)]
    pub results: Vec<ToolResult>,
    #[serde(default)]
    pub reasoning_result: Option<ReasoningResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct InvestigationLoop {
    pub budget: InvestigationBudget,
    #[serde(default)]
    pub iterations: Vec<InvestigationIteration>,
    pub stop_reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct EvidencePacket {
    pub investigation_id: String,
    pub question: String,
    #[serde(default)]
    pub targets: Vec<Target>,
    #[serde(default)]
    pub alerts: Vec<Alert>,
    #[serde(default)]
    pub evidence: Vec<Evidence>,
    #[serde(default)]
    pub runbooks: Vec<Runbook>,
    pub constraints: InvestigationConstraints,
    pub redaction: RedactionReport,
    #[serde(default)]
    pub metadata: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct InvestigationConstraints {
    pub read_only: bool,
    pub no_command_execution: bool,
    pub no_production_mutation: bool,
    pub advisory_only: bool,
    #[serde(default)]
    pub notes: Vec<String>,
}

impl Default for InvestigationConstraints {
    fn default() -> Self {
        Self {
            read_only: true,
            no_command_execution: true,
            no_production_mutation: true,
            advisory_only: true,
            notes: vec![
                "Vigil must not execute commands.".to_string(),
                "Recommended checks must be descriptive and read-only.".to_string(),
            ],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct RedactionReport {
    pub applied: bool,
    pub masked_values: usize,
    #[serde(default)]
    pub warnings: Vec<String>,
}

impl Default for RedactionReport {
    fn default() -> Self {
        Self {
            applied: true,
            masked_values: 0,
            warnings: vec![
                "Basic redaction is best-effort; review inputs before sending them to an LLM."
                    .to_string(),
            ],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct ReasoningResult {
    pub summary: String,
    pub hypotheses: Vec<Hypothesis>,
    pub missing_checks: Vec<MissingCheck>,
    pub recommended_checks: Vec<RecommendedCheck>,
    pub risk_notes: Vec<String>,
    pub operator_notes: Vec<String>,
    pub confidence_notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct Hypothesis {
    pub id: String,
    pub title: String,
    pub description: String,
    pub confidence: f32,
    #[serde(default)]
    pub supporting_evidence_ids: Vec<String>,
    #[serde(default)]
    pub contradicting_evidence_ids: Vec<String>,
    #[serde(default)]
    pub missing_checks: Vec<String>,
    pub risk_if_wrong: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct MissingCheck {
    pub id: String,
    pub title: String,
    pub description: String,
    #[serde(default)]
    pub target: Option<String>,
    pub reason: String,
    #[serde(default)]
    pub related_evidence_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct RecommendedCheck {
    pub id: String,
    pub title: String,
    pub description: String,
    #[serde(default)]
    pub target: Option<String>,
    pub reason: String,
    pub read_only: bool,
    pub source: String,
    #[serde(default)]
    pub related_evidence_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct EvidenceBrief {
    pub title: String,
    pub summary: String,
    #[serde(default)]
    pub targets: Vec<Target>,
    #[serde(default)]
    pub evidence: Vec<Evidence>,
    #[serde(default)]
    pub hypotheses: Vec<Hypothesis>,
    #[serde(default)]
    pub missing_checks: Vec<MissingCheck>,
    #[serde(default)]
    pub recommended_checks: Vec<RecommendedCheck>,
    #[serde(default)]
    pub risk_notes: Vec<String>,
    #[serde(default)]
    pub references: Vec<SourceReference>,
    #[serde(default)]
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct Trajectory {
    pub id: String,
    pub started_at: String,
    pub completed_at: String,
    pub inputs: TrajectoryInputs,
    #[serde(default)]
    pub sources: Vec<Source>,
    #[serde(default)]
    pub capabilities: Vec<Capability>,
    #[serde(default)]
    pub investigation_loop: Option<InvestigationLoop>,
    #[serde(default)]
    pub resolved_targets: Vec<Target>,
    pub evidence_packet: EvidencePacket,
    #[serde(default)]
    pub reasoning_result: Option<ReasoningResult>,
    pub brief: EvidenceBrief,
    #[serde(default)]
    pub llm: Option<LlmExchangeMetadata>,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default)]
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq, Default)]
pub struct TrajectoryInputs {
    #[serde(default)]
    pub case_dir: Option<String>,
    #[serde(default)]
    pub alert: Option<String>,
    #[serde(default)]
    pub inventory: Option<String>,
    #[serde(default)]
    pub runbooks: Vec<String>,
    #[serde(default)]
    pub runbook_dir: Option<String>,
    #[serde(default)]
    pub target: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct LlmExchangeMetadata {
    pub provider: String,
    pub model: String,
    #[serde(default)]
    pub request_id: Option<String>,
    #[serde(default)]
    pub response_metadata: BTreeMap<String, String>,
}

pub fn reasoning_result_schema() -> Result<Value, ModelError> {
    serde_json::to_value(schema_for!(ReasoningResult)).map_err(ModelError::InvalidJson)
}

pub fn tool_plan_schema() -> Result<Value, ModelError> {
    serde_json::to_value(schema_for!(ToolPlan)).map_err(ModelError::InvalidJson)
}

pub fn parse_reasoning_result_str(content: &str) -> Result<ReasoningResult, ModelError> {
    let json_text = strip_json_fence(content);
    let value: Value = serde_json::from_str(&json_text)?;
    parse_reasoning_result_value(&value)
}

pub fn parse_tool_plan_str(content: &str) -> Result<ToolPlan, ModelError> {
    let json_text = strip_json_fence(content);
    let value: Value = serde_json::from_str(&json_text)?;
    parse_tool_plan_value(&value)
}

pub fn parse_reasoning_result_value(value: &Value) -> Result<ReasoningResult, ModelError> {
    validate_reasoning_schema(value)?;
    let result: ReasoningResult = serde_json::from_value(value.clone())?;
    validate_reasoning_result(&result)?;
    Ok(result)
}

pub fn parse_tool_plan_value(value: &Value) -> Result<ToolPlan, ModelError> {
    validate_tool_plan_schema(value)?;
    let plan: ToolPlan = serde_json::from_value(value.clone())?;
    validate_tool_plan_model(&plan)?;
    Ok(plan)
}

pub fn validate_reasoning_schema(value: &Value) -> Result<(), ModelError> {
    let schema = reasoning_result_schema()?;
    let compiled = jsonschema::JSONSchema::compile(&schema)
        .map_err(|err| ModelError::SchemaCompile(err.to_string()))?;

    if let Err(errors) = compiled.validate(value) {
        let messages = errors
            .map(|err| err.to_string())
            .collect::<Vec<_>>()
            .join("; ");
        return Err(ModelError::SchemaValidation(messages));
    }

    Ok(())
}

pub fn validate_tool_plan_schema(value: &Value) -> Result<(), ModelError> {
    let schema = tool_plan_schema()?;
    let compiled = jsonschema::JSONSchema::compile(&schema)
        .map_err(|err| ModelError::SchemaCompile(err.to_string()))?;

    if let Err(errors) = compiled.validate(value) {
        let messages = errors
            .map(|err| err.to_string())
            .collect::<Vec<_>>()
            .join("; ");
        return Err(ModelError::SchemaValidation(messages));
    }

    Ok(())
}

pub fn validate_tool_plan_model(plan: &ToolPlan) -> Result<(), ModelError> {
    let mut errors = Vec::new();

    if plan.id.trim().is_empty() {
        errors.push("tool_plan.id must not be empty".to_string());
    }
    if plan.rationale.trim().is_empty() {
        errors.push(format!(
            "tool_plan '{}' rationale must not be empty",
            plan.id
        ));
    }

    let mut seen_call_ids = BTreeSet::new();
    for call in &plan.calls {
        if call.id.trim().is_empty() {
            errors.push("tool_call.id must not be empty".to_string());
        }
        if !seen_call_ids.insert(call.id.clone()) {
            errors.push(format!("tool_call '{}' is duplicated", call.id));
        }
        if call.capability_id.trim().is_empty() {
            errors.push(format!(
                "tool_call '{}' capability_id must not be empty",
                call.id
            ));
        }
        if call.source_id.trim().is_empty() {
            errors.push(format!(
                "tool_call '{}' source_id must not be empty",
                call.id
            ));
        }
        if call.reason.trim().is_empty() {
            errors.push(format!("tool_call '{}' reason must not be empty", call.id));
        }
        if contains_command_like_text(&call.reason) {
            errors.push(format!(
                "tool_call '{}' reason must describe a read-only check without runnable shell commands",
                call.id
            ));
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(ModelError::SemanticValidation(errors.join("; ")))
    }
}

pub fn validate_reasoning_result(result: &ReasoningResult) -> Result<(), ModelError> {
    let mut errors = Vec::new();

    if result.summary.trim().is_empty() {
        errors.push("summary must not be empty".to_string());
    }

    let mut evidence_refs = BTreeSet::new();
    for hypothesis in &result.hypotheses {
        if hypothesis.id.trim().is_empty() {
            errors.push("hypothesis.id must not be empty".to_string());
        }
        if hypothesis.title.trim().is_empty() {
            errors.push(format!(
                "hypothesis '{}' title must not be empty",
                hypothesis.id
            ));
        }
        if !(0.0..=1.0).contains(&hypothesis.confidence) {
            errors.push(format!(
                "hypothesis '{}' confidence must be between 0.0 and 1.0",
                hypothesis.id
            ));
        }
        evidence_refs.extend(hypothesis.supporting_evidence_ids.iter().cloned());
        evidence_refs.extend(hypothesis.contradicting_evidence_ids.iter().cloned());
    }

    for missing in &result.missing_checks {
        if missing.id.trim().is_empty() {
            errors.push("missing_check.id must not be empty".to_string());
        }
        if missing.title.trim().is_empty() {
            errors.push(format!(
                "missing_check '{}' title must not be empty",
                missing.id
            ));
        }
        evidence_refs.extend(missing.related_evidence_ids.iter().cloned());
    }

    for check in &result.recommended_checks {
        if check.id.trim().is_empty() {
            errors.push("recommended_check.id must not be empty".to_string());
        }
        if check.title.trim().is_empty() {
            errors.push(format!(
                "recommended_check '{}' title must not be empty",
                check.id
            ));
        }
        if !check.read_only {
            errors.push(format!(
                "recommended_check '{}' must be marked read_only",
                check.id
            ));
        }
        if contains_command_like_text(&check.title)
            || contains_command_like_text(&check.description)
        {
            errors.push(format!(
                "recommended_check '{}' must describe a read-only check without runnable shell commands",
                check.id
            ));
        }
        evidence_refs.extend(check.related_evidence_ids.iter().cloned());
    }

    if evidence_refs.iter().any(|id| id.trim().is_empty()) {
        errors.push("evidence reference IDs must not be empty".to_string());
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(ModelError::SemanticValidation(errors.join("; ")))
    }
}

pub fn contains_command_like_text(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.starts_with("```") || trimmed.starts_with("$ ") || trimmed.contains("\n$ ") {
        return true;
    }

    let lower = trimmed.to_ascii_lowercase();
    let command_prefixes = [
        "ansible ",
        "bash ",
        "curl ",
        "docker ",
        "helm ",
        "kubectl ",
        "mysql ",
        "psql ",
        "python ",
        "sh ",
        "ssh ",
        "sudo ",
        "systemctl ",
        "terraform ",
    ];

    command_prefixes
        .iter()
        .any(|prefix| lower.starts_with(prefix))
}

fn strip_json_fence(content: &str) -> String {
    let trimmed = content.trim();
    if !trimmed.starts_with("```") {
        return trimmed.to_string();
    }

    let mut lines = trimmed.lines();
    let first = lines.next();
    if first.is_none() {
        return trimmed.to_string();
    }

    let mut body = lines.collect::<Vec<_>>();
    if body.last().is_some_and(|line| line.trim() == "```") {
        body.pop();
    }
    body.join("\n").trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_reasoning_json() -> Value {
        serde_json::json!({
            "summary": "The web service is returning elevated 5xx responses.",
            "hypotheses": [{
                "id": "hyp-1",
                "title": "Backend dependency errors",
                "description": "Recent dependency failures could explain the symptom.",
                "confidence": 0.6,
                "supporting_evidence_ids": ["alert:web-5xx"],
                "contradicting_evidence_ids": [],
                "missing_checks": ["missing-1"],
                "risk_if_wrong": "Operators may focus on the wrong dependency first."
            }],
            "missing_checks": [{
                "id": "missing-1",
                "title": "Recent deploy status",
                "description": "Deployment status has not been supplied.",
                "target": "service:web",
                "reason": "Recent changes are common incident drivers.",
                "related_evidence_ids": ["alert:web-5xx"]
            }],
            "recommended_checks": [{
                "id": "check-1",
                "title": "Review web service error-rate dashboard",
                "description": "Compare the alert window with service-level error-rate and latency charts.",
                "target": "service:web",
                "reason": "This verifies the alert scope without changing production.",
                "read_only": true,
                "source": "llm",
                "related_evidence_ids": ["alert:web-5xx"]
            }],
            "risk_notes": ["Hypotheses are advisory."],
            "operator_notes": ["Recommended checks were not executed."],
            "confidence_notes": ["Confidence is limited by missing deploy data."]
        })
    }

    fn valid_tool_plan_json() -> Value {
        serde_json::json!({
            "id": "plan-1",
            "rationale": "Collect read-only telemetry and changes.",
            "calls": [{
                "id": "tool-1",
                "capability_id": "prometheus_query",
                "source_id": "prometheus:prod",
                "target": "service:web",
                "since": "30m",
                "reason": "Confirm recent error-rate telemetry.",
                "inputs": {
                    "query": "sum(rate(http_requests_total[5m]))"
                }
            }]
        })
    }

    #[test]
    fn serializes_models() -> Result<(), Box<dyn std::error::Error>> {
        let target = Target {
            id: "service:web".to_string(),
            kind: TargetKind::Service,
            name: "web".to_string(),
            environment: Some("prod".to_string()),
            service: Some("web".to_string()),
            host: None,
            labels: BTreeMap::new(),
            criticality: Some("high".to_string()),
            metadata: BTreeMap::new(),
        };

        let json = serde_json::to_string(&target)?;
        assert!(json.contains("service:web"));
        let decoded: Target = serde_json::from_str(&json)?;
        assert_eq!(decoded.kind, TargetKind::Service);
        Ok(())
    }

    #[test]
    fn validates_reasoning_result_schema_and_semantics() -> Result<(), Box<dyn std::error::Error>> {
        let reasoning = parse_reasoning_result_value(&valid_reasoning_json())?;
        assert_eq!(reasoning.recommended_checks.len(), 1);
        Ok(())
    }

    #[test]
    fn rejects_non_read_only_recommended_checks() {
        let mut value = valid_reasoning_json();
        value["recommended_checks"][0]["read_only"] = Value::Bool(false);

        let error = parse_reasoning_result_value(&value).err();
        assert!(matches!(
            error,
            Some(ModelError::SemanticValidation(message))
                if message.contains("must be marked read_only")
        ));
    }

    #[test]
    fn rejects_runnable_shell_command_text() {
        let mut value = valid_reasoning_json();
        value["recommended_checks"][0]["description"] =
            Value::String("kubectl get pods -n prod".to_string());

        let error = parse_reasoning_result_value(&value).err();
        assert!(matches!(
            error,
            Some(ModelError::SemanticValidation(message))
                if message.contains("without runnable shell commands")
        ));
    }

    #[test]
    fn validates_tool_plan_schema_and_semantics() -> Result<(), Box<dyn std::error::Error>> {
        let plan = parse_tool_plan_value(&valid_tool_plan_json())?;
        assert_eq!(plan.calls.len(), 1);
        assert_eq!(plan.calls[0].capability_id, "prometheus_query");
        Ok(())
    }

    #[test]
    fn rejects_command_like_tool_plan_reason() {
        let mut value = valid_tool_plan_json();
        value["calls"][0]["reason"] = Value::String("kubectl get pods".to_string());

        let error = parse_tool_plan_value(&value).err();
        assert!(matches!(
            error,
            Some(ModelError::SemanticValidation(message))
                if message.contains("without runnable shell commands")
        ));
    }
}
