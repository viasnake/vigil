use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    net::ToSocketAddrs,
    path::{Path, PathBuf},
    time::Instant,
};

use chrono::{Duration as ChronoDuration, Utc};
use serde_json::{json, Value};
use thiserror::Error;
use uuid::Uuid;
use vigil_llm::{LlmError, LlmProvider, ProviderResponse};
use vigil_model::{
    validate_reasoning_result, validate_tool_plan_model, Alert, Capability, CapabilityKind,
    CaseManifest, Evidence, EvidenceBrief, EvidenceKind, EvidencePacket, EvidenceSource,
    Hypothesis, Inventory, InvestigationBudget, InvestigationConstraints, InvestigationIteration,
    InvestigationLoop, LlmExchangeMetadata, MissingCheck, ReasoningResult, RecommendedCheck,
    RedactionReport, Runbook, Source, SourceKind, SourceReference, Target, TargetKind, ToolCall,
    ToolPlan, ToolResult, ToolResultStatus, Trajectory, TrajectoryInputs,
};

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("{kind} file '{path}' could not be read: {source}")]
    ReadInput {
        kind: &'static str,
        path: String,
        source: std::io::Error,
    },
    #[error("{kind} file '{path}' is not valid JSON: {source}")]
    ParseJson {
        kind: &'static str,
        path: String,
        source: serde_json::Error,
    },
    #[error("{kind} file '{path}' is not valid YAML: {source}")]
    ParseYaml {
        kind: &'static str,
        path: String,
        source: serde_yaml::Error,
    },
    #[error("{kind} file '{path}' failed validation: {errors}")]
    Validation {
        kind: &'static str,
        path: String,
        errors: String,
    },
    #[error("runbook directory '{path}' could not be read: {source}")]
    ReadRunbookDir {
        path: String,
        source: std::io::Error,
    },
    #[error("{kind} directory '{path}' could not be read: {source}")]
    ReadDirectory {
        kind: &'static str,
        path: String,
        source: std::io::Error,
    },
    #[error("runbook directory '{path}' is not a directory")]
    RunbookDirNotDirectory { path: String },
    #[error("case directory '{path}' already exists. Use --force to overwrite the manifest.")]
    CaseAlreadyExists { path: String },
    #[error("case directory '{path}' does not contain vigil.yaml. Run 'vigil case init' first.")]
    MissingCaseManifest { path: String },
    #[error("case directory '{path}' is missing required subdirectory '{subdir}'")]
    MissingCaseSubdir { path: String, subdir: &'static str },
    #[error("case path '{path}' exists but is not a directory")]
    CasePathNotDirectory { path: String },
    #[error("case file '{path}' could not be written: {source}")]
    WriteCaseFile {
        path: String,
        source: std::io::Error,
    },
    #[error("case directory '{path}' could not be created: {source}")]
    CreateCaseDir {
        path: String,
        source: std::io::Error,
    },
    #[error("runbook '{source_path}' could not be copied to '{destination}': {error}")]
    CopyRunbook {
        source_path: String,
        destination: String,
        error: String,
    },
    #[error("YAML could not be generated for {kind}: {source}")]
    SerializeYaml {
        kind: &'static str,
        source: serde_yaml::Error,
    },
    #[error("investigation requires a provider unless --no-llm or --dry-run is used")]
    MissingProvider,
    #[error("LLM provider failed: {0}")]
    Llm(#[from] LlmError),
    #[error("redaction failed while rebuilding the evidence packet: {0}")]
    Redaction(String),
    #[error("deterministic reasoning result failed validation: {0}")]
    DeterministicReasoning(String),
    #[error("investigation plan failed policy validation: {0}")]
    PolicyValidation(String),
}

#[derive(Debug, Clone)]
pub struct InvestigationRequest {
    pub alert_path: Option<PathBuf>,
    pub inventory_path: PathBuf,
    pub runbook_paths: Vec<PathBuf>,
    pub runbook_dir: Option<PathBuf>,
    pub target: Option<String>,
    pub no_llm: bool,
    pub dry_run: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ValidationRequest {
    pub alert_path: Option<PathBuf>,
    pub inventory_path: Option<PathBuf>,
    pub runbook_paths: Vec<PathBuf>,
    pub runbook_dir: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct InvestigationOutcome {
    pub brief: EvidenceBrief,
    pub trajectory: Trajectory,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvestigationSelector {
    Target(String),
    Alert(String),
}

impl InvestigationSelector {
    pub fn label(&self) -> &str {
        match self {
            Self::Target(value) | Self::Alert(value) => value,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AgentInvestigationRequest {
    pub selector: InvestigationSelector,
    pub since: Option<String>,
    pub sources: Vec<SourceConfig>,
    pub source_filters: Vec<String>,
    pub budget: InvestigationBudget,
    pub no_llm: bool,
    pub dry_run: bool,
    pub plan_only: bool,
}

#[derive(Debug, Clone)]
pub enum SourceConfig {
    InventoryFile {
        name: String,
        path: Option<PathBuf>,
    },
    RunbookFile {
        name: String,
        dir: Option<PathBuf>,
        paths: Vec<PathBuf>,
    },
    Alertmanager {
        name: String,
        url: Option<String>,
        fixture_path: Option<PathBuf>,
        bearer_token_env: Option<String>,
    },
    Prometheus {
        name: String,
        url: Option<String>,
        fixture_path: Option<PathBuf>,
        bearer_token_env: Option<String>,
    },
    Github {
        name: String,
        api_url: Option<String>,
        repo: Option<String>,
        fixture_path: Option<PathBuf>,
        bearer_token_env: Option<String>,
    },
    Http {
        name: String,
        url: Option<String>,
        fixture_path: Option<PathBuf>,
        bearer_token_env: Option<String>,
    },
    Dns {
        name: String,
        fixture_path: Option<PathBuf>,
    },
    Loki {
        name: String,
        url: Option<String>,
        fixture_path: Option<PathBuf>,
        bearer_token_env: Option<String>,
    },
    Grafana {
        name: String,
        url: Option<String>,
        fixture_path: Option<PathBuf>,
        bearer_token_env: Option<String>,
    },
    Kubernetes {
        name: String,
        url: Option<String>,
        namespace: Option<String>,
        fixture_path: Option<PathBuf>,
        bearer_token_env: Option<String>,
    },
}

#[derive(Debug, Clone)]
pub struct PlanOnlyOutcome {
    pub plan: ToolPlan,
    pub sources: Vec<Source>,
    pub capabilities: Vec<Capability>,
    pub warnings: Vec<String>,
}

impl SourceConfig {
    fn source_kind(&self) -> SourceKind {
        match self {
            Self::InventoryFile { .. } => SourceKind::InventoryFile,
            Self::RunbookFile { .. } => SourceKind::RunbookFile,
            Self::Alertmanager { .. } => SourceKind::Alertmanager,
            Self::Prometheus { .. } => SourceKind::Prometheus,
            Self::Github { .. } => SourceKind::Github,
            Self::Http { .. } => SourceKind::Http,
            Self::Dns { .. } => SourceKind::Dns,
            Self::Loki { .. } => SourceKind::Loki,
            Self::Grafana { .. } => SourceKind::Grafana,
            Self::Kubernetes { .. } => SourceKind::Kubernetes,
        }
    }

    fn name(&self) -> &str {
        match self {
            Self::InventoryFile { name, .. }
            | Self::RunbookFile { name, .. }
            | Self::Alertmanager { name, .. }
            | Self::Prometheus { name, .. }
            | Self::Github { name, .. }
            | Self::Http { name, .. }
            | Self::Dns { name, .. }
            | Self::Loki { name, .. }
            | Self::Grafana { name, .. }
            | Self::Kubernetes { name, .. } => name,
        }
    }

    fn source_id(&self) -> String {
        format!("{}:{}", self.source_kind().as_str(), self.name())
    }

    fn matches_filter(&self, filter: &str) -> bool {
        let source_id = self.source_id();
        let kind = self.source_kind();
        filter == source_id
            || filter == self.name()
            || filter == kind.as_str()
            || filter
                .split_once(':')
                .is_some_and(|(filter_kind, filter_name)| {
                    filter_kind == kind.as_str() && filter_name == self.name()
                })
    }
}

#[derive(Debug, Clone)]
struct PreparedAgentInvestigation {
    investigation_id: String,
    packet: EvidencePacket,
    resolved_targets: Vec<Target>,
    source_configs: Vec<SourceConfig>,
    sources: Vec<Source>,
    capabilities: Vec<Capability>,
    warnings: Vec<String>,
    primary_inventory_path: Option<String>,
    primary_runbook_dir: Option<String>,
    runbook_paths: Vec<String>,
    llm_metadata: Option<LlmExchangeMetadata>,
}

#[derive(Debug, Clone)]
pub struct CaseInitRequest {
    pub case_dir: PathBuf,
    pub target: String,
    pub severity: String,
    pub summary: String,
    pub force: bool,
}

#[derive(Debug, Clone)]
pub struct EvidenceAddRequest {
    pub case_dir: PathBuf,
    pub kind: EvidenceKind,
    pub summary: String,
    pub source: String,
    pub url: Option<String>,
    pub file: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct ChangeAddRequest {
    pub case_dir: PathBuf,
    pub summary: String,
    pub source: String,
    pub url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RunbookAddRequest {
    pub case_dir: PathBuf,
    pub runbook_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct CaseInvestigationRequest {
    pub case_dir: PathBuf,
    pub no_llm: bool,
    pub dry_run: bool,
}

#[derive(Debug, Clone)]
pub struct AddedEvidence {
    pub path: PathBuf,
    pub evidence: Evidence,
}

pub async fn investigate(
    request: InvestigationRequest,
    provider: Option<&dyn LlmProvider>,
) -> Result<InvestigationOutcome, CoreError> {
    let started_at = Utc::now().to_rfc3339();
    let investigation_id = Uuid::now_v7().to_string();

    let inventory = load_inventory(&request.inventory_path)?;
    let alert = match &request.alert_path {
        Some(path) => Some(load_alert(path)?),
        None => None,
    };
    let runbooks = load_runbooks(&request.runbook_paths, request.runbook_dir.as_deref())?;
    let resolved_targets = resolve_targets(alert.as_ref(), &inventory, request.target.as_deref());
    let evidence = build_evidence(alert.as_ref(), &inventory, &resolved_targets, &runbooks)?;
    let question =
        investigation_question(alert.as_ref(), request.target.as_deref(), &resolved_targets);

    let packet = EvidencePacket {
        investigation_id: investigation_id.clone(),
        question,
        targets: resolved_targets.clone(),
        alerts: alert.iter().cloned().collect(),
        evidence,
        runbooks: matching_runbooks(&runbooks, &resolved_targets),
        constraints: InvestigationConstraints::default(),
        redaction: RedactionReport::default(),
        metadata: BTreeMap::from([
            ("tool".to_string(), Value::String("vigil".to_string())),
            (
                "mode".to_string(),
                Value::String(investigation_mode(&request).to_string()),
            ),
        ]),
    };
    let packet = redact_evidence_packet(packet)?;

    let mut warnings = packet.redaction.warnings.clone();
    if request.no_llm {
        warnings.push(
            "--no-llm was used; reasoning is deterministic and not LLM-assisted.".to_string(),
        );
    }
    if request.dry_run {
        warnings.push("--dry-run was used; no LLM request was sent.".to_string());
    }

    let (reasoning_result, llm_metadata) = if request.no_llm || request.dry_run {
        (deterministic_reasoning(&packet)?, None)
    } else {
        let provider = provider.ok_or(CoreError::MissingProvider)?;
        let response = provider.reason(&packet).await?;
        provider_response_parts(response)
    };

    let brief = build_brief(&packet, &reasoning_result, &warnings);
    let completed_at = Utc::now().to_rfc3339();
    let trajectory = Trajectory {
        id: investigation_id,
        started_at,
        completed_at,
        inputs: TrajectoryInputs {
            case_dir: None,
            alert: request.alert_path.as_deref().map(display_path),
            inventory: Some(display_path(&request.inventory_path)),
            runbooks: request
                .runbook_paths
                .iter()
                .map(|path| display_path(path))
                .collect(),
            runbook_dir: request.runbook_dir.as_deref().map(display_path),
            target: request.target.clone(),
        },
        sources: Vec::new(),
        capabilities: Vec::new(),
        investigation_loop: None,
        resolved_targets,
        evidence_packet: packet,
        reasoning_result: Some(reasoning_result),
        brief: brief.clone(),
        llm: llm_metadata,
        warnings,
        errors: Vec::new(),
    };

    Ok(InvestigationOutcome { brief, trajectory })
}

pub async fn plan_agent_investigation(
    request: AgentInvestigationRequest,
    provider: Option<&dyn LlmProvider>,
) -> Result<PlanOnlyOutcome, CoreError> {
    let prepared = prepare_agent_investigation(&request)?;
    let (plan, _planning_reasoning, _planning_metadata) = plan_next_read_only_actions(
        &prepared.packet,
        &prepared.capabilities,
        &request,
        provider,
        1,
    )
    .await?;
    validate_tool_plan(&plan, &prepared.sources, &prepared.capabilities)?;

    Ok(PlanOnlyOutcome {
        plan,
        sources: prepared.sources,
        capabilities: prepared.capabilities,
        warnings: prepared.warnings,
    })
}

pub async fn investigate_agent(
    request: AgentInvestigationRequest,
    provider: Option<&dyn LlmProvider>,
) -> Result<InvestigationOutcome, CoreError> {
    let started_at = Utc::now().to_rfc3339();
    let mut prepared = prepare_agent_investigation(&request)?;
    let investigation_id = prepared.investigation_id.clone();
    let resolved_targets = prepared.resolved_targets.clone();
    let mut warnings = prepared.warnings.clone();
    let mut loop_record = InvestigationLoop {
        budget: request.budget.clone(),
        iterations: Vec::new(),
        stop_reason: "investigation loop did not run".to_string(),
    };
    let loop_started = Instant::now();
    let mut executed_keys = BTreeSet::new();
    let mut total_tool_calls = 0_u32;

    if request.plan_only {
        loop_record.stop_reason = "plan-only mode requested; no tools were executed".to_string();
    } else {
        for iteration in 1..=request.budget.max_iterations {
            if loop_started.elapsed().as_secs() >= request.budget.max_duration_secs {
                loop_record.stop_reason = "duration budget exhausted".to_string();
                break;
            }
            if total_tool_calls >= request.budget.max_tool_calls {
                loop_record.stop_reason = "tool-call budget exhausted".to_string();
                break;
            }

            let (mut plan, planning_reasoning, planning_metadata) = plan_next_read_only_actions(
                &prepared.packet,
                &prepared.capabilities,
                &request,
                provider,
                iteration,
            )
            .await?;
            if let Some(metadata) = planning_metadata {
                prepared.llm_metadata = Some(metadata);
            }

            plan.calls.retain(|call| {
                let key = tool_call_dedupe_key(call);
                !executed_keys.contains(&key)
            });
            let remaining = (request.budget.max_tool_calls - total_tool_calls) as usize;
            plan.calls.truncate(remaining);
            validate_tool_plan(&plan, &prepared.sources, &prepared.capabilities)?;

            if plan.calls.is_empty() {
                loop_record.stop_reason = "no useful new read-only checks remain".to_string();
                loop_record.iterations.push(InvestigationIteration {
                    index: iteration,
                    plan,
                    results: Vec::new(),
                    reasoning_result: planning_reasoning,
                });
                break;
            }

            let mut results = Vec::new();
            let mut collected_evidence = Vec::new();
            for call in &plan.calls {
                executed_keys.insert(tool_call_dedupe_key(call));
                let result = execute_read_only_tool(call, &prepared.source_configs).await?;
                if result.status == ToolResultStatus::Succeeded {
                    collected_evidence.extend(result.evidence.clone());
                }
                results.push(result);
            }

            total_tool_calls += plan.calls.len() as u32;
            let collected_count = collected_evidence.len();
            prepared.packet.evidence.extend(collected_evidence);
            prepared.packet = redact_evidence_packet(prepared.packet)?;

            loop_record.iterations.push(InvestigationIteration {
                index: iteration,
                plan,
                results,
                reasoning_result: planning_reasoning,
            });

            if collected_count == 0 {
                loop_record.stop_reason =
                    "read-only checks produced no additional evidence".to_string();
                break;
            }
            if iteration == request.budget.max_iterations {
                loop_record.stop_reason = "iteration budget exhausted".to_string();
            }
        }
    }

    warnings.extend(prepared.packet.redaction.warnings.clone());
    if request.no_llm {
        warnings.push("--no-llm was used; planning and reasoning are deterministic.".to_string());
    }
    if request.dry_run {
        warnings.push("--dry-run was used; no LLM request was sent.".to_string());
    }

    let (reasoning_result, llm_metadata) = if request.no_llm || request.dry_run {
        (
            deterministic_reasoning(&prepared.packet)?,
            prepared.llm_metadata,
        )
    } else {
        let provider = provider.ok_or(CoreError::MissingProvider)?;
        let response = provider.reason(&prepared.packet).await?;
        provider_response_parts(response)
    };

    let brief = build_brief(&prepared.packet, &reasoning_result, &warnings);
    let completed_at = Utc::now().to_rfc3339();
    let trajectory = Trajectory {
        id: investigation_id,
        started_at,
        completed_at,
        inputs: TrajectoryInputs {
            case_dir: None,
            alert: match &request.selector {
                InvestigationSelector::Alert(name) => Some(name.clone()),
                InvestigationSelector::Target(_) => None,
            },
            inventory: prepared.primary_inventory_path.clone(),
            runbooks: prepared.runbook_paths.clone(),
            runbook_dir: prepared.primary_runbook_dir.clone(),
            target: match &request.selector {
                InvestigationSelector::Target(target) => Some(target.clone()),
                InvestigationSelector::Alert(_) => None,
            },
        },
        sources: prepared.sources,
        capabilities: prepared.capabilities,
        investigation_loop: Some(loop_record),
        resolved_targets,
        evidence_packet: prepared.packet,
        reasoning_result: Some(reasoning_result),
        brief: brief.clone(),
        llm: llm_metadata,
        warnings: dedup_strings(warnings),
        errors: Vec::new(),
    };

    Ok(InvestigationOutcome { brief, trajectory })
}

pub fn init_case(request: CaseInitRequest) -> Result<CaseManifest, CoreError> {
    if request.case_dir.exists() {
        if !request.force {
            return Err(CoreError::CaseAlreadyExists {
                path: request.case_dir.display().to_string(),
            });
        }
        if !request.case_dir.is_dir() {
            return Err(CoreError::CasePathNotDirectory {
                path: request.case_dir.display().to_string(),
            });
        }
    }

    create_case_subdir(&request.case_dir)?;
    create_case_subdir(&request.case_dir.join("evidence"))?;
    create_case_subdir(&request.case_dir.join("runbooks"))?;
    create_case_subdir(&request.case_dir.join("output"))?;

    let id = case_id_from_dir(&request.case_dir);
    let manifest = CaseManifest {
        title: case_title_from_id(&id),
        id,
        severity: request.severity,
        status: "investigating".to_string(),
        target: request.target,
        summary: request.summary,
        created_at: Utc::now().to_rfc3339(),
    };
    validate_case_manifest(&manifest, &request.case_dir)?;
    write_yaml_file(
        &case_manifest_path(&request.case_dir),
        &manifest,
        "case manifest",
    )?;
    Ok(manifest)
}

pub fn load_case_manifest(case_dir: &Path) -> Result<CaseManifest, CoreError> {
    let path = case_manifest_path(case_dir);
    if !path.exists() {
        return Err(CoreError::MissingCaseManifest {
            path: case_dir.display().to_string(),
        });
    }
    let manifest: CaseManifest = parse_document("case manifest", &path)?;
    validate_case_manifest(&manifest, &path)?;
    Ok(manifest)
}

pub fn add_case_evidence(request: EvidenceAddRequest) -> Result<AddedEvidence, CoreError> {
    let manifest = load_case_manifest(&request.case_dir)?;
    let evidence_dir = required_case_subdir(&request.case_dir, "evidence")?;
    let evidence_path = next_case_evidence_path(&evidence_dir, &request.kind)?;
    let evidence = build_case_evidence_item(
        &manifest,
        request.kind,
        evidence_path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("evidence"),
        &request.summary,
        &request.source,
        request.url.as_deref(),
        request.file.as_deref(),
    )?;
    validate_evidence(&evidence, &evidence_path)?;
    write_yaml_file(&evidence_path, &evidence, "case evidence")?;
    Ok(AddedEvidence {
        path: evidence_path,
        evidence,
    })
}

pub fn add_case_change(request: ChangeAddRequest) -> Result<AddedEvidence, CoreError> {
    add_case_evidence(EvidenceAddRequest {
        case_dir: request.case_dir,
        kind: EvidenceKind::Change,
        summary: request.summary,
        source: request.source,
        url: request.url,
        file: None,
    })
}

pub fn add_case_runbook(request: RunbookAddRequest) -> Result<PathBuf, CoreError> {
    let _manifest = load_case_manifest(&request.case_dir)?;
    let runbook = load_runbook(&request.runbook_path)?;
    let runbook_dir = required_case_subdir(&request.case_dir, "runbooks")?;
    let file_name = request
        .runbook_path
        .file_name()
        .ok_or_else(|| CoreError::Validation {
            kind: "runbook",
            path: request.runbook_path.display().to_string(),
            errors: "runbook path must include a file name".to_string(),
        })?;
    let destination = runbook_dir.join(file_name);
    if request.runbook_path != destination {
        fs::copy(&request.runbook_path, &destination).map_err(|err| CoreError::CopyRunbook {
            source_path: request.runbook_path.display().to_string(),
            destination: destination.display().to_string(),
            error: err.to_string(),
        })?;
    }
    let _validated = runbook;
    Ok(destination)
}

pub async fn investigate_case(
    request: CaseInvestigationRequest,
    provider: Option<&dyn LlmProvider>,
) -> Result<InvestigationOutcome, CoreError> {
    let started_at = Utc::now().to_rfc3339();
    let investigation_id = Uuid::now_v7().to_string();
    let manifest = load_case_manifest(&request.case_dir)?;
    let evidence_dir = required_case_subdir(&request.case_dir, "evidence")?;
    let runbook_dir = required_case_subdir(&request.case_dir, "runbooks")?;
    let supplied_evidence = load_case_evidence_dir(&evidence_dir)?;
    let runbook_paths = supported_document_paths("runbook", &runbook_dir)?;
    let runbooks = load_runbooks(&runbook_paths, None)?;
    let target = target_from_case_manifest(&manifest);
    let alert = alert_from_case_manifest(&manifest);
    let evidence = build_case_evidence(&manifest, &alert, &target, supplied_evidence, &runbooks)?;
    let question = format!("Investigate case '{}': {}", manifest.id, manifest.summary);

    let packet = EvidencePacket {
        investigation_id: investigation_id.clone(),
        question,
        targets: vec![target.clone()],
        alerts: vec![alert],
        evidence,
        runbooks: matching_runbooks(&runbooks, std::slice::from_ref(&target)),
        constraints: InvestigationConstraints::default(),
        redaction: RedactionReport::default(),
        metadata: BTreeMap::from([
            ("tool".to_string(), Value::String("vigil".to_string())),
            ("mode".to_string(), Value::String("case".to_string())),
            ("case_id".to_string(), Value::String(manifest.id.clone())),
        ]),
    };
    let packet = redact_evidence_packet(packet)?;

    let mut warnings = packet.redaction.warnings.clone();
    if request.no_llm {
        warnings.push(
            "--no-llm was used; reasoning is deterministic and not LLM-assisted.".to_string(),
        );
    }
    if request.dry_run {
        warnings.push("--dry-run was used; no LLM request was sent.".to_string());
    }

    let (reasoning_result, llm_metadata) = if request.no_llm || request.dry_run {
        (deterministic_reasoning(&packet)?, None)
    } else {
        let provider = provider.ok_or(CoreError::MissingProvider)?;
        let response = provider.reason(&packet).await?;
        provider_response_parts(response)
    };

    let brief = build_brief(&packet, &reasoning_result, &warnings);
    let completed_at = Utc::now().to_rfc3339();
    let trajectory = Trajectory {
        id: investigation_id,
        started_at,
        completed_at,
        inputs: TrajectoryInputs {
            case_dir: Some(display_path(&request.case_dir)),
            alert: None,
            inventory: None,
            runbooks: runbook_paths
                .iter()
                .map(|path| display_path(path))
                .collect(),
            runbook_dir: Some(display_path(&runbook_dir)),
            target: Some(manifest.target),
        },
        sources: Vec::new(),
        capabilities: Vec::new(),
        investigation_loop: None,
        resolved_targets: vec![target],
        evidence_packet: packet,
        reasoning_result: Some(reasoning_result),
        brief: brief.clone(),
        llm: llm_metadata,
        warnings,
        errors: Vec::new(),
    };

    Ok(InvestigationOutcome { brief, trajectory })
}

pub fn validate_input_files(request: ValidationRequest) -> Result<(), CoreError> {
    if let Some(path) = request.inventory_path {
        let _inventory = load_inventory(&path)?;
    }
    if let Some(path) = request.alert_path {
        let _alert = load_alert(&path)?;
    }
    let _runbooks = load_runbooks(&request.runbook_paths, request.runbook_dir.as_deref())?;
    Ok(())
}

pub fn load_trajectory(path: &Path) -> Result<Trajectory, CoreError> {
    let text = read_to_string("trajectory", path)?;
    serde_json::from_str(&text).map_err(|source| CoreError::ParseJson {
        kind: "trajectory",
        path: path.display().to_string(),
        source,
    })
}

fn prepare_agent_investigation(
    request: &AgentInvestigationRequest,
) -> Result<PreparedAgentInvestigation, CoreError> {
    let investigation_id = Uuid::now_v7().to_string();
    let source_configs = filtered_or_default_sources(request);
    let sources = source_configs
        .iter()
        .map(source_from_config)
        .collect::<Vec<_>>();
    let capabilities = source_configs
        .iter()
        .map(capability_from_config)
        .collect::<Vec<_>>();
    let mut warnings = Vec::new();

    let inventory = load_agent_inventory(&source_configs, &mut warnings)?;
    let runbooks = load_agent_runbooks(&source_configs, &mut warnings)?;
    let alert = match &request.selector {
        InvestigationSelector::Alert(name) => Some(selector_alert(name)),
        InvestigationSelector::Target(_) => None,
    };
    let resolved_targets = match &request.selector {
        InvestigationSelector::Target(target) => {
            resolve_targets(alert.as_ref(), &inventory, Some(target))
        }
        InvestigationSelector::Alert(name) => {
            let targets = resolve_targets(alert.as_ref(), &inventory, None);
            if targets.is_empty() {
                vec![unknown_target(&format!("alert:{name}"))]
            } else {
                targets
            }
        }
    };
    let evidence = build_evidence(alert.as_ref(), &inventory, &resolved_targets, &runbooks)?;
    let packet = EvidencePacket {
        investigation_id: investigation_id.clone(),
        question: agent_investigation_question(&request.selector, request.since.as_deref()),
        targets: resolved_targets.clone(),
        alerts: alert.iter().cloned().collect(),
        evidence,
        runbooks: matching_runbooks(&runbooks, &resolved_targets),
        constraints: InvestigationConstraints::default(),
        redaction: RedactionReport::default(),
        metadata: BTreeMap::from([
            ("tool".to_string(), Value::String("vigil".to_string())),
            ("mode".to_string(), Value::String("agent".to_string())),
            (
                "selector".to_string(),
                Value::String(request.selector.label().to_string()),
            ),
            (
                "selector_kind".to_string(),
                Value::String(match &request.selector {
                    InvestigationSelector::Target(_) => "target".to_string(),
                    InvestigationSelector::Alert(_) => "alert".to_string(),
                }),
            ),
            (
                "since".to_string(),
                Value::String(
                    request
                        .since
                        .clone()
                        .unwrap_or_else(|| "unspecified".to_string()),
                ),
            ),
        ]),
    };
    let packet = redact_evidence_packet(packet)?;
    warnings.extend(packet.redaction.warnings.clone());

    Ok(PreparedAgentInvestigation {
        investigation_id,
        packet,
        resolved_targets,
        primary_inventory_path: first_inventory_path(&source_configs),
        primary_runbook_dir: first_runbook_dir(&source_configs),
        runbook_paths: configured_runbook_paths(&source_configs),
        source_configs,
        sources,
        capabilities,
        warnings,
        llm_metadata: None,
    })
}

fn filtered_or_default_sources(request: &AgentInvestigationRequest) -> Vec<SourceConfig> {
    let mut sources = if request.sources.is_empty() {
        default_agent_sources()
    } else {
        request.sources.clone()
    };

    if !request.source_filters.is_empty() {
        sources.retain(|source| {
            request
                .source_filters
                .iter()
                .any(|filter| source.matches_filter(filter))
        });
    }

    sources
}

fn default_agent_sources() -> Vec<SourceConfig> {
    vec![
        SourceConfig::InventoryFile {
            name: "local".to_string(),
            path: Some(PathBuf::from("inventory.yaml")),
        },
        SourceConfig::RunbookFile {
            name: "local".to_string(),
            dir: Some(PathBuf::from("runbooks")),
            paths: Vec::new(),
        },
        SourceConfig::Alertmanager {
            name: "default".to_string(),
            url: None,
            fixture_path: None,
            bearer_token_env: None,
        },
        SourceConfig::Prometheus {
            name: "default".to_string(),
            url: None,
            fixture_path: None,
            bearer_token_env: None,
        },
        SourceConfig::Github {
            name: "default".to_string(),
            api_url: None,
            repo: None,
            fixture_path: None,
            bearer_token_env: None,
        },
    ]
}

fn source_from_config(config: &SourceConfig) -> Source {
    Source {
        id: config.source_id(),
        kind: config.source_kind(),
        name: config.name().to_string(),
        read_only: true,
        config: redacted_source_config(config),
    }
}

fn redacted_source_config(config: &SourceConfig) -> BTreeMap<String, Value> {
    match config {
        SourceConfig::InventoryFile { path, .. } => optional_path_config("path", path.as_deref()),
        SourceConfig::RunbookFile { dir, paths, .. } => {
            let mut values = optional_path_config("dir", dir.as_deref());
            if !paths.is_empty() {
                values.insert(
                    "paths".to_string(),
                    Value::Array(
                        paths
                            .iter()
                            .map(|path| Value::String(path.display().to_string()))
                            .collect(),
                    ),
                );
            }
            values
        }
        SourceConfig::Alertmanager {
            url,
            fixture_path,
            bearer_token_env,
            ..
        }
        | SourceConfig::Prometheus {
            url,
            fixture_path,
            bearer_token_env,
            ..
        }
        | SourceConfig::Http {
            url,
            fixture_path,
            bearer_token_env,
            ..
        }
        | SourceConfig::Loki {
            url,
            fixture_path,
            bearer_token_env,
            ..
        }
        | SourceConfig::Grafana {
            url,
            fixture_path,
            bearer_token_env,
            ..
        } => {
            let mut values = BTreeMap::new();
            if let Some(url) = url {
                values.insert("url".to_string(), Value::String(url.clone()));
            }
            if let Some(path) = fixture_path {
                values.insert(
                    "fixture_path".to_string(),
                    Value::String(path.display().to_string()),
                );
            }
            if let Some(env_name) = bearer_token_env {
                values.insert(
                    "bearer_token_env".to_string(),
                    Value::String(env_name.clone()),
                );
            }
            values
        }
        SourceConfig::Github {
            api_url,
            repo,
            fixture_path,
            bearer_token_env,
            ..
        } => {
            let mut values = BTreeMap::new();
            if let Some(api_url) = api_url {
                values.insert("api_url".to_string(), Value::String(api_url.clone()));
            }
            if let Some(repo) = repo {
                values.insert("repo".to_string(), Value::String(repo.clone()));
            }
            if let Some(path) = fixture_path {
                values.insert(
                    "fixture_path".to_string(),
                    Value::String(path.display().to_string()),
                );
            }
            if let Some(env_name) = bearer_token_env {
                values.insert(
                    "bearer_token_env".to_string(),
                    Value::String(env_name.clone()),
                );
            }
            values
        }
        SourceConfig::Dns { fixture_path, .. } => {
            let mut values = BTreeMap::new();
            if let Some(path) = fixture_path {
                values.insert(
                    "fixture_path".to_string(),
                    Value::String(path.display().to_string()),
                );
            }
            values
        }
        SourceConfig::Kubernetes {
            url,
            namespace,
            fixture_path,
            bearer_token_env,
            ..
        } => {
            let mut values = BTreeMap::new();
            if let Some(url) = url {
                values.insert("url".to_string(), Value::String(url.clone()));
            }
            if let Some(namespace) = namespace {
                values.insert("namespace".to_string(), Value::String(namespace.clone()));
            }
            if let Some(path) = fixture_path {
                values.insert(
                    "fixture_path".to_string(),
                    Value::String(path.display().to_string()),
                );
            }
            if let Some(env_name) = bearer_token_env {
                values.insert(
                    "bearer_token_env".to_string(),
                    Value::String(env_name.clone()),
                );
            }
            values
        }
    }
}

fn optional_path_config(key: &str, path: Option<&Path>) -> BTreeMap<String, Value> {
    path.map(|path| BTreeMap::from([(key.to_string(), Value::String(display_path(path)))]))
        .unwrap_or_default()
}

fn capability_from_config(config: &SourceConfig) -> Capability {
    let (id, kind, description, input_schema) = match config {
        SourceConfig::InventoryFile { .. } => (
            "inventory_lookup",
            CapabilityKind::Inventory,
            "Resolve target metadata from a local inventory file.",
            BTreeMap::from([("target".to_string(), "string".to_string())]),
        ),
        SourceConfig::RunbookFile { .. } => (
            "runbook_lookup",
            CapabilityKind::Runbook,
            "Find matching local runbooks and read-only checks.",
            BTreeMap::from([("target".to_string(), "string".to_string())]),
        ),
        SourceConfig::Alertmanager { .. } => (
            "alertmanager_active_alerts",
            CapabilityKind::Alerts,
            "Read active alerts and alert metadata.",
            BTreeMap::from([("matcher".to_string(), "string".to_string())]),
        ),
        SourceConfig::Prometheus { .. } => (
            "prometheus_query",
            CapabilityKind::Metrics,
            "Read Prometheus metric data for the investigation window.",
            BTreeMap::from([
                ("query".to_string(), "string".to_string()),
                ("since".to_string(), "duration".to_string()),
            ]),
        ),
        SourceConfig::Github { .. } => (
            "github_recent_changes",
            CapabilityKind::Changes,
            "Read recent GitHub changes for a configured repository.",
            BTreeMap::from([
                ("repo".to_string(), "string".to_string()),
                ("since".to_string(), "duration".to_string()),
            ]),
        ),
        SourceConfig::Http { .. } => (
            "http_check",
            CapabilityKind::Http,
            "Read an HTTP endpoint status and response metadata.",
            BTreeMap::from([
                ("url".to_string(), "string".to_string()),
                ("method".to_string(), "string".to_string()),
            ]),
        ),
        SourceConfig::Dns { .. } => (
            "dns_lookup",
            CapabilityKind::Dns,
            "Resolve DNS addresses for a target host.",
            BTreeMap::from([("host".to_string(), "string".to_string())]),
        ),
        SourceConfig::Loki { .. } => (
            "loki_query_range",
            CapabilityKind::Logs,
            "Read Loki log entries for the investigation window.",
            BTreeMap::from([
                ("query".to_string(), "string".to_string()),
                ("since".to_string(), "duration".to_string()),
                ("limit".to_string(), "integer".to_string()),
            ]),
        ),
        SourceConfig::Grafana { .. } => (
            "grafana_annotations",
            CapabilityKind::Dashboards,
            "Read Grafana annotations for the investigation window.",
            BTreeMap::from([
                ("tags".to_string(), "array".to_string()),
                ("since".to_string(), "duration".to_string()),
            ]),
        ),
        SourceConfig::Kubernetes { .. } => (
            "kubernetes_events",
            CapabilityKind::Kubernetes,
            "Read Kubernetes events from a configured API endpoint.",
            BTreeMap::from([
                ("namespace".to_string(), "string".to_string()),
                ("field_selector".to_string(), "string".to_string()),
                ("limit".to_string(), "integer".to_string()),
            ]),
        ),
    };

    Capability {
        id: id.to_string(),
        kind,
        source_id: config.source_id(),
        adapter: config.source_kind(),
        read_only: true,
        description: description.to_string(),
        input_schema,
        risk: "low".to_string(),
    }
}

fn load_agent_inventory(
    configs: &[SourceConfig],
    warnings: &mut Vec<String>,
) -> Result<Inventory, CoreError> {
    for config in configs {
        let SourceConfig::InventoryFile {
            path: Some(path), ..
        } = config
        else {
            continue;
        };
        if path.is_file() {
            return load_inventory(path);
        }
        warnings.push(format!(
            "Inventory source '{}' did not find readable file '{}'.",
            config.source_id(),
            path.display()
        ));
    }

    warnings
        .push("No inventory file source loaded; target resolution may be incomplete.".to_string());
    Ok(Inventory::default())
}

fn load_agent_runbooks(
    configs: &[SourceConfig],
    warnings: &mut Vec<String>,
) -> Result<Vec<Runbook>, CoreError> {
    let mut all_runbooks = Vec::new();
    for config in configs {
        let SourceConfig::RunbookFile { dir, paths, .. } = config else {
            continue;
        };
        if let Some(dir) = dir {
            if dir.is_dir() {
                all_runbooks.extend(load_runbooks(paths, Some(dir))?);
            } else {
                warnings.push(format!(
                    "Runbook source '{}' did not find directory '{}'.",
                    config.source_id(),
                    dir.display()
                ));
                all_runbooks.extend(load_runbooks(paths, None)?);
            }
        } else {
            all_runbooks.extend(load_runbooks(paths, None)?);
        }
    }
    Ok(all_runbooks)
}

fn first_inventory_path(configs: &[SourceConfig]) -> Option<String> {
    configs.iter().find_map(|config| match config {
        SourceConfig::InventoryFile {
            path: Some(path), ..
        } => Some(display_path(path)),
        _ => None,
    })
}

fn first_runbook_dir(configs: &[SourceConfig]) -> Option<String> {
    configs.iter().find_map(|config| match config {
        SourceConfig::RunbookFile {
            dir: Some(path), ..
        } => Some(display_path(path)),
        _ => None,
    })
}

fn configured_runbook_paths(configs: &[SourceConfig]) -> Vec<String> {
    configs
        .iter()
        .flat_map(|config| match config {
            SourceConfig::RunbookFile { paths, .. } => paths
                .iter()
                .map(|path| display_path(path))
                .collect::<Vec<_>>(),
            _ => Vec::new(),
        })
        .collect()
}

fn selector_alert(name: &str) -> Alert {
    Alert {
        id: slug_id(name),
        name: name.to_string(),
        severity: "unknown".to_string(),
        status: "requested".to_string(),
        summary: format!("Investigation requested for alert '{name}'."),
        description: None,
        target: None,
        started_at: None,
        ended_at: None,
        labels: BTreeMap::from([("alertname".to_string(), name.to_string())]),
        annotations: BTreeMap::new(),
        source: Some("selector".to_string()),
    }
}

fn slug_id(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

fn agent_investigation_question(selector: &InvestigationSelector, since: Option<&str>) -> String {
    let window = since
        .map(|since| format!(" over the last {since}"))
        .unwrap_or_default();
    match selector {
        InvestigationSelector::Target(target) => {
            format!("Investigate target '{target}'{window} with bounded read-only tools.")
        }
        InvestigationSelector::Alert(alert) => {
            format!("Investigate alert '{alert}'{window} with bounded read-only tools.")
        }
    }
}

async fn plan_next_read_only_actions(
    packet: &EvidencePacket,
    capabilities: &[Capability],
    request: &AgentInvestigationRequest,
    provider: Option<&dyn LlmProvider>,
    iteration: u32,
) -> Result<
    (
        ToolPlan,
        Option<ReasoningResult>,
        Option<LlmExchangeMetadata>,
    ),
    CoreError,
> {
    if request.no_llm || request.dry_run || request.plan_only {
        return Ok((
            deterministic_tool_plan(packet, capabilities, request, iteration),
            None,
            None,
        ));
    }

    let Some(provider) = provider else {
        return Ok((
            deterministic_tool_plan(packet, capabilities, request, iteration),
            None,
            None,
        ));
    };

    let response = provider.plan(packet, capabilities).await?;
    let mut plan = response.plan;
    if plan.id.trim().is_empty() {
        plan.id = format!("plan-{iteration}");
    }
    for (index, call) in plan.calls.iter_mut().enumerate() {
        if call.id.trim().is_empty() {
            call.id = format!("iter-{iteration}-tool-{}", index + 1);
        }
        if call.since.is_none() {
            call.since = request.since.clone();
        }
        if call.target.is_none() {
            call.target = packet.targets.first().map(|target| target.id.clone());
        }
    }
    let plan = if plan.calls.is_empty() && plan.rationale.trim().is_empty() {
        deterministic_tool_plan(packet, capabilities, request, iteration)
    } else {
        plan
    };

    Ok((plan, None, Some(response.metadata)))
}

fn deterministic_tool_plan(
    packet: &EvidencePacket,
    capabilities: &[Capability],
    request: &AgentInvestigationRequest,
    iteration: u32,
) -> ToolPlan {
    let target_id = packet
        .targets
        .first()
        .map(|target| target.id.clone())
        .unwrap_or_else(|| request.selector.label().to_string());
    let mut calls = Vec::new();

    push_capability_call(
        &mut calls,
        capabilities,
        "inventory_lookup",
        &target_id,
        request.since.as_deref(),
        "Resolve current target metadata and dependencies.",
        BTreeMap::from([("target".to_string(), Value::String(target_id.clone()))]),
    );
    push_capability_call(
        &mut calls,
        capabilities,
        "runbook_lookup",
        &target_id,
        request.since.as_deref(),
        "Ground the investigation in matching read-only runbook checks.",
        BTreeMap::from([("target".to_string(), Value::String(target_id.clone()))]),
    );
    if let InvestigationSelector::Alert(alert_name) = &request.selector {
        push_capability_call(
            &mut calls,
            capabilities,
            "alertmanager_active_alerts",
            &target_id,
            request.since.as_deref(),
            "Check whether the requested alert is active and inspect its labels.",
            BTreeMap::from([(
                "matcher".to_string(),
                Value::String(format!("alertname={alert_name}")),
            )]),
        );
    } else {
        push_capability_call(
            &mut calls,
            capabilities,
            "alertmanager_active_alerts",
            &target_id,
            request.since.as_deref(),
            "Check active alerts for the target.",
            BTreeMap::from([("matcher".to_string(), Value::String(target_id.clone()))]),
        );
    }
    push_capability_call(
        &mut calls,
        capabilities,
        "prometheus_query",
        &target_id,
        request.since.as_deref(),
        "Check recent error-rate telemetry for the target.",
        BTreeMap::from([(
            "query".to_string(),
            Value::String(default_error_rate_query(&target_id)),
        )]),
    );
    push_capability_call(
        &mut calls,
        capabilities,
        "prometheus_query",
        &target_id,
        request.since.as_deref(),
        "Check recent latency telemetry for the target.",
        BTreeMap::from([(
            "query".to_string(),
            Value::String(default_latency_query(&target_id)),
        )]),
    );
    push_capability_call(
        &mut calls,
        capabilities,
        "github_recent_changes",
        &target_id,
        request.since.as_deref(),
        "Check recent repository changes that may correlate with the symptom.",
        BTreeMap::from([("target".to_string(), Value::String(target_id.clone()))]),
    );
    push_capability_call(
        &mut calls,
        capabilities,
        "http_check",
        &target_id,
        request.since.as_deref(),
        "Check configured HTTP endpoint health without mutating production.",
        BTreeMap::from([("target".to_string(), Value::String(target_id.clone()))]),
    );
    push_capability_call(
        &mut calls,
        capabilities,
        "dns_lookup",
        &target_id,
        request.since.as_deref(),
        "Resolve DNS for the target to check routing context.",
        BTreeMap::from([(
            "host".to_string(),
            Value::String(target_name_for_query(&target_id)),
        )]),
    );
    push_capability_call(
        &mut calls,
        capabilities,
        "loki_query_range",
        &target_id,
        request.since.as_deref(),
        "Check recent logs for target-related errors.",
        BTreeMap::from([(
            "query".to_string(),
            Value::String(default_loki_query(&target_id)),
        )]),
    );
    push_capability_call(
        &mut calls,
        capabilities,
        "grafana_annotations",
        &target_id,
        request.since.as_deref(),
        "Check Grafana annotations for recent operational events.",
        BTreeMap::from([(
            "tags".to_string(),
            Value::Array(vec![Value::String(target_name_for_query(&target_id))]),
        )]),
    );
    push_capability_call(
        &mut calls,
        capabilities,
        "kubernetes_events",
        &target_id,
        request.since.as_deref(),
        "Check Kubernetes events for workload-level context.",
        BTreeMap::from([("target".to_string(), Value::String(target_id.clone()))]),
    );

    let calls = calls
        .into_iter()
        .enumerate()
        .map(|(index, mut call)| {
            call.id = format!("iter-{iteration}-tool-{}", index + 1);
            call
        })
        .collect();

    ToolPlan {
        id: format!("plan-{iteration}"),
        rationale:
            "Deterministic planner selected registered read-only SRE investigation capabilities."
                .to_string(),
        calls,
    }
}

fn push_capability_call(
    calls: &mut Vec<ToolCall>,
    capabilities: &[Capability],
    capability_id: &str,
    target: &str,
    since: Option<&str>,
    reason: &str,
    inputs: BTreeMap<String, Value>,
) {
    let Some(capability) = capabilities.iter().find(|item| item.id == capability_id) else {
        return;
    };
    calls.push(ToolCall {
        id: String::new(),
        capability_id: capability.id.clone(),
        source_id: capability.source_id.clone(),
        target: Some(target.to_string()),
        since: since.map(ToOwned::to_owned),
        reason: reason.to_string(),
        inputs,
    });
}

fn default_error_rate_query(target_id: &str) -> String {
    let service = target_name_for_query(target_id);
    format!("sum(rate(http_requests_total{{service=\"{service}\",status=~\"5..\"}}[5m]))")
}

fn default_latency_query(target_id: &str) -> String {
    let service = target_name_for_query(target_id);
    format!(
        "histogram_quantile(0.95, sum(rate(http_request_duration_seconds_bucket{{service=\"{service}\"}}[5m])) by (le))"
    )
}

fn default_loki_query(target_id: &str) -> String {
    let service = target_name_for_query(target_id);
    format!("{{service=\"{service}\"}}")
}

fn target_name_for_query(target_id: &str) -> String {
    target_id
        .split_once(':')
        .map(|(_, value)| value)
        .unwrap_or(target_id)
        .replace('"', "")
}

fn validate_tool_plan(
    plan: &ToolPlan,
    sources: &[Source],
    capabilities: &[Capability],
) -> Result<(), CoreError> {
    validate_tool_plan_model(plan).map_err(|err| CoreError::PolicyValidation(err.to_string()))?;
    let mut errors = Vec::new();
    for call in &plan.calls {
        let Some(capability) = capabilities.iter().find(|capability| {
            capability.id == call.capability_id && capability.source_id == call.source_id
        }) else {
            errors.push(format!(
                "tool call '{}' references unregistered capability '{}' on source '{}'",
                call.id, call.capability_id, call.source_id
            ));
            continue;
        };
        if !capability.read_only {
            errors.push(format!(
                "tool call '{}' references non-read-only capability '{}'",
                call.id, call.capability_id
            ));
        }
        let Some(source) = sources.iter().find(|source| source.id == call.source_id) else {
            errors.push(format!(
                "tool call '{}' references unregistered source '{}'",
                call.id, call.source_id
            ));
            continue;
        };
        if !source.read_only {
            errors.push(format!(
                "tool call '{}' references non-read-only source '{}'",
                call.id, call.source_id
            ));
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(CoreError::PolicyValidation(errors.join("; ")))
    }
}

async fn execute_read_only_tool(
    call: &ToolCall,
    configs: &[SourceConfig],
) -> Result<ToolResult, CoreError> {
    let started_at = Utc::now().to_rfc3339();
    let result = match configs
        .iter()
        .find(|config| config.source_id() == call.source_id)
    {
        Some(SourceConfig::InventoryFile { path, .. }) => execute_inventory_lookup(call, path),
        Some(SourceConfig::RunbookFile { dir, paths, .. }) => {
            execute_runbook_lookup(call, dir.as_deref(), paths)
        }
        Some(SourceConfig::Alertmanager {
            url,
            fixture_path,
            bearer_token_env,
            ..
        }) => {
            execute_alertmanager_lookup(
                call,
                url.as_deref(),
                fixture_path.as_deref(),
                bearer_token_env.as_deref(),
            )
            .await
        }
        Some(SourceConfig::Prometheus {
            url,
            fixture_path,
            bearer_token_env,
            ..
        }) => {
            execute_prometheus_query(
                call,
                url.as_deref(),
                fixture_path.as_deref(),
                bearer_token_env.as_deref(),
            )
            .await
        }
        Some(SourceConfig::Github {
            api_url,
            repo,
            fixture_path,
            bearer_token_env,
            ..
        }) => {
            execute_github_recent_changes(
                call,
                api_url.as_deref(),
                repo.as_deref(),
                fixture_path.as_deref(),
                bearer_token_env.as_deref(),
            )
            .await
        }
        Some(SourceConfig::Http {
            url,
            fixture_path,
            bearer_token_env,
            ..
        }) => {
            execute_http_check(
                call,
                url.as_deref(),
                fixture_path.as_deref(),
                bearer_token_env.as_deref(),
            )
            .await
        }
        Some(SourceConfig::Dns { fixture_path, .. }) => {
            execute_dns_lookup(call, fixture_path.as_deref())
        }
        Some(SourceConfig::Loki {
            url,
            fixture_path,
            bearer_token_env,
            ..
        }) => {
            execute_loki_query_range(
                call,
                url.as_deref(),
                fixture_path.as_deref(),
                bearer_token_env.as_deref(),
            )
            .await
        }
        Some(SourceConfig::Grafana {
            url,
            fixture_path,
            bearer_token_env,
            ..
        }) => {
            execute_grafana_annotations(
                call,
                url.as_deref(),
                fixture_path.as_deref(),
                bearer_token_env.as_deref(),
            )
            .await
        }
        Some(SourceConfig::Kubernetes {
            url,
            namespace,
            fixture_path,
            bearer_token_env,
            ..
        }) => {
            execute_kubernetes_events(
                call,
                url.as_deref(),
                namespace.as_deref(),
                fixture_path.as_deref(),
                bearer_token_env.as_deref(),
            )
            .await
        }
        None => Ok((
            ToolResultStatus::Failed,
            Vec::new(),
            Some("source is not registered".to_string()),
        )),
    }?;
    let completed_at = Utc::now().to_rfc3339();
    Ok(ToolResult {
        call_id: call.id.clone(),
        capability_id: call.capability_id.clone(),
        source_id: call.source_id.clone(),
        status: result.0,
        started_at,
        completed_at,
        evidence: result.1,
        error: result.2,
    })
}

type AdapterExecution = Result<(ToolResultStatus, Vec<Evidence>, Option<String>), CoreError>;

fn execute_inventory_lookup(call: &ToolCall, path: &Option<PathBuf>) -> AdapterExecution {
    let Some(path) = path else {
        return Ok(skipped_result(
            "inventory-file source has no path configured",
        ));
    };
    if !path.is_file() {
        return Ok(skipped_result(&format!(
            "inventory file '{}' does not exist",
            path.display()
        )));
    }
    let inventory = load_inventory(path)?;
    let target_selector = call
        .target
        .as_deref()
        .or_else(|| call.inputs.get("target").and_then(Value::as_str));
    let targets = resolve_targets(None, &inventory, target_selector);
    let evidence = build_evidence(None, &inventory, &targets, &[])?;
    Ok((ToolResultStatus::Succeeded, evidence, None))
}

fn execute_runbook_lookup(
    call: &ToolCall,
    dir: Option<&Path>,
    paths: &[PathBuf],
) -> AdapterExecution {
    if dir.is_none() && paths.is_empty() {
        return Ok(skipped_result(
            "runbook-file source has neither dir nor paths configured",
        ));
    }
    if let Some(dir) = dir {
        if !dir.is_dir() {
            return Ok(skipped_result(&format!(
                "runbook directory '{}' does not exist",
                dir.display()
            )));
        }
    }
    let runbooks = load_runbooks(paths, dir)?;
    let target = call
        .target
        .as_deref()
        .map(unknown_target)
        .unwrap_or_else(|| unknown_target("unknown"));
    let evidence = build_evidence(None, &Inventory::default(), &[target], &runbooks)?;
    Ok((ToolResultStatus::Succeeded, evidence, None))
}

async fn execute_alertmanager_lookup(
    call: &ToolCall,
    url: Option<&str>,
    fixture_path: Option<&Path>,
    bearer_token_env: Option<&str>,
) -> AdapterExecution {
    let (alerts, source_path) = if let Some(path) = fixture_path {
        (load_alerts_fixture(path)?, Some(display_path(path)))
    } else if let Some(url) = url {
        match fetch_alertmanager_alerts(url, call, bearer_token_env).await {
            Ok(value) => (parse_alertmanager_alerts(value)?, None),
            Err(message) => return Ok(failed_result(&message)),
        }
    } else {
        return Ok(skipped_result(
            "alertmanager source has no url or fixture_path configured",
        ));
    };
    let matcher = call.inputs.get("matcher").and_then(Value::as_str);
    let filtered = alerts
        .into_iter()
        .filter(|alert| alert_matches(alert, matcher, call.target.as_deref()))
        .collect::<Vec<_>>();
    let evidence = filtered
        .iter()
        .map(|alert| {
            alert_to_evidence(alert, "alertmanager", source_path.as_deref().map(Path::new))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok((ToolResultStatus::Succeeded, evidence, None))
}

async fn execute_prometheus_query(
    call: &ToolCall,
    url: Option<&str>,
    fixture_path: Option<&Path>,
    bearer_token_env: Option<&str>,
) -> AdapterExecution {
    let query = call
        .inputs
        .get("query")
        .and_then(Value::as_str)
        .unwrap_or("up");
    let (data, source_path, source_url) = if let Some(path) = fixture_path {
        (
            parse_document("prometheus fixture", path)?,
            Some(display_path(path)),
            None,
        )
    } else if let Some(url) = url {
        match fetch_prometheus_query_range(url, call, query, bearer_token_env).await {
            Ok(value) => (value, None, Some(url.to_string())),
            Err(message) => return Ok(failed_result(&message)),
        }
    } else {
        return Ok(skipped_result(
            "prometheus source has no url or fixture_path configured",
        ));
    };
    let evidence = Evidence {
        id: format!("metric:{}:{}", call.id, slug_id(query)),
        kind: EvidenceKind::Metric,
        summary: format!("Prometheus returned data for query '{query}'."),
        source: EvidenceSource {
            kind: "prometheus".to_string(),
            name: call.source_id.clone(),
            path: source_path.or(source_url),
        },
        target: call.target.clone(),
        timestamp: Some(Utc::now().to_rfc3339()),
        confidence: 0.8,
        data: json!({
            "query": query,
            "since": call.since,
            "response": data
        }),
        references: Vec::new(),
    };
    Ok((ToolResultStatus::Succeeded, vec![evidence], None))
}

async fn execute_github_recent_changes(
    call: &ToolCall,
    api_url: Option<&str>,
    repo: Option<&str>,
    fixture_path: Option<&Path>,
    bearer_token_env: Option<&str>,
) -> AdapterExecution {
    let repo = call.inputs.get("repo").and_then(Value::as_str).or(repo);
    let (data, source_path) = if let Some(path) = fixture_path {
        (
            parse_document("github fixture", path)?,
            Some(display_path(path)),
        )
    } else if let Some(repo) = repo {
        match fetch_github_commits(api_url, repo, call, bearer_token_env).await {
            Ok(value) => (value, None),
            Err(message) => return Ok(failed_result(&message)),
        }
    } else {
        return Ok(skipped_result(
            "github source has no repo or fixture_path configured",
        ));
    };
    let summary = repo
        .map(|repo| format!("GitHub supplied recent changes for '{repo}'."))
        .unwrap_or_else(|| "GitHub supplied recent changes.".to_string());
    let evidence = Evidence {
        id: format!("change:{}", call.id),
        kind: EvidenceKind::Change,
        summary,
        source: EvidenceSource {
            kind: "github".to_string(),
            name: call.source_id.clone(),
            path: source_path.or_else(|| repo.map(|repo| format!("github:{repo}"))),
        },
        target: call.target.clone(),
        timestamp: Some(Utc::now().to_rfc3339()),
        confidence: 0.8,
        data: json!({
            "repo": repo,
            "since": call.since,
            "response": data
        }),
        references: Vec::new(),
    };
    Ok((ToolResultStatus::Succeeded, vec![evidence], None))
}

async fn execute_http_check(
    call: &ToolCall,
    url: Option<&str>,
    fixture_path: Option<&Path>,
    bearer_token_env: Option<&str>,
) -> AdapterExecution {
    let (data, source_path) = if let Some(path) = fixture_path {
        (
            parse_document("http fixture", path)?,
            Some(display_path(path)),
        )
    } else if let Some(url) = call.inputs.get("url").and_then(Value::as_str).or(url) {
        match fetch_http_check(url, bearer_token_env).await {
            Ok(value) => (value, Some(url.to_string())),
            Err(message) => return Ok(failed_result(&message)),
        }
    } else {
        return Ok(skipped_result(
            "http source has no url or fixture_path configured",
        ));
    };

    let status = data.get("status").and_then(Value::as_u64);
    let evidence = Evidence {
        id: format!("http:{}", call.id),
        kind: EvidenceKind::External,
        summary: status
            .map(|status| format!("HTTP check returned status {status}."))
            .unwrap_or_else(|| "HTTP check returned data.".to_string()),
        source: EvidenceSource {
            kind: "http".to_string(),
            name: call.source_id.clone(),
            path: source_path,
        },
        target: call.target.clone(),
        timestamp: Some(Utc::now().to_rfc3339()),
        confidence: 0.8,
        data,
        references: Vec::new(),
    };
    Ok((ToolResultStatus::Succeeded, vec![evidence], None))
}

fn execute_dns_lookup(call: &ToolCall, fixture_path: Option<&Path>) -> AdapterExecution {
    let (data, source_path) = if let Some(path) = fixture_path {
        (
            parse_document("dns fixture", path)?,
            Some(display_path(path)),
        )
    } else {
        let Some(host) = call
            .inputs
            .get("host")
            .and_then(Value::as_str)
            .or(call.target.as_deref())
            .map(target_name_for_query)
        else {
            return Ok(skipped_result("dns lookup has no host or target"));
        };
        match resolve_dns_host(&host) {
            Ok(value) => (value, Some(format!("dns:{host}"))),
            Err(message) => return Ok(failed_result(&message)),
        }
    };
    let evidence = Evidence {
        id: format!("dns:{}", call.id),
        kind: EvidenceKind::External,
        summary: "DNS lookup returned address data.".to_string(),
        source: EvidenceSource {
            kind: "dns".to_string(),
            name: call.source_id.clone(),
            path: source_path,
        },
        target: call.target.clone(),
        timestamp: Some(Utc::now().to_rfc3339()),
        confidence: 0.8,
        data,
        references: Vec::new(),
    };
    Ok((ToolResultStatus::Succeeded, vec![evidence], None))
}

async fn execute_loki_query_range(
    call: &ToolCall,
    url: Option<&str>,
    fixture_path: Option<&Path>,
    bearer_token_env: Option<&str>,
) -> AdapterExecution {
    let default_query = call
        .target
        .as_deref()
        .map(default_loki_query)
        .unwrap_or_else(|| "{}".to_string());
    let query = call
        .inputs
        .get("query")
        .and_then(Value::as_str)
        .unwrap_or(default_query.as_str());
    let (data, source_path) = if let Some(path) = fixture_path {
        (
            parse_document("loki fixture", path)?,
            Some(display_path(path)),
        )
    } else if let Some(url) = url {
        match fetch_loki_query_range(url, call, query, bearer_token_env).await {
            Ok(value) => (value, Some(url.to_string())),
            Err(message) => return Ok(failed_result(&message)),
        }
    } else {
        return Ok(skipped_result(
            "loki source has no url or fixture_path configured",
        ));
    };
    let evidence = Evidence {
        id: format!("log:{}", call.id),
        kind: EvidenceKind::Log,
        summary: format!("Loki returned log data for query '{query}'."),
        source: EvidenceSource {
            kind: "loki".to_string(),
            name: call.source_id.clone(),
            path: source_path,
        },
        target: call.target.clone(),
        timestamp: Some(Utc::now().to_rfc3339()),
        confidence: 0.8,
        data: json!({
            "query": query,
            "since": call.since,
            "response": data
        }),
        references: Vec::new(),
    };
    Ok((ToolResultStatus::Succeeded, vec![evidence], None))
}

async fn execute_grafana_annotations(
    call: &ToolCall,
    url: Option<&str>,
    fixture_path: Option<&Path>,
    bearer_token_env: Option<&str>,
) -> AdapterExecution {
    let (data, source_path) = if let Some(path) = fixture_path {
        (
            parse_document("grafana fixture", path)?,
            Some(display_path(path)),
        )
    } else if let Some(url) = url {
        match fetch_grafana_annotations(url, call, bearer_token_env).await {
            Ok(value) => (value, Some(url.to_string())),
            Err(message) => return Ok(failed_result(&message)),
        }
    } else {
        return Ok(skipped_result(
            "grafana source has no url or fixture_path configured",
        ));
    };
    let evidence = Evidence {
        id: format!("grafana:{}", call.id),
        kind: EvidenceKind::Change,
        summary: "Grafana returned annotation data for the investigation window.".to_string(),
        source: EvidenceSource {
            kind: "grafana".to_string(),
            name: call.source_id.clone(),
            path: source_path,
        },
        target: call.target.clone(),
        timestamp: Some(Utc::now().to_rfc3339()),
        confidence: 0.8,
        data,
        references: Vec::new(),
    };
    Ok((ToolResultStatus::Succeeded, vec![evidence], None))
}

async fn execute_kubernetes_events(
    call: &ToolCall,
    url: Option<&str>,
    namespace: Option<&str>,
    fixture_path: Option<&Path>,
    bearer_token_env: Option<&str>,
) -> AdapterExecution {
    let namespace = call
        .inputs
        .get("namespace")
        .and_then(Value::as_str)
        .or(namespace);
    let (data, source_path) = if let Some(path) = fixture_path {
        (
            parse_document("kubernetes fixture", path)?,
            Some(display_path(path)),
        )
    } else if let Some(url) = url {
        match fetch_kubernetes_events(url, namespace, call, bearer_token_env).await {
            Ok(value) => (value, Some(url.to_string())),
            Err(message) => return Ok(failed_result(&message)),
        }
    } else {
        return Ok(skipped_result(
            "kubernetes source has no url or fixture_path configured",
        ));
    };
    let evidence = Evidence {
        id: format!("kubernetes:{}", call.id),
        kind: EvidenceKind::External,
        summary: "Kubernetes returned event data.".to_string(),
        source: EvidenceSource {
            kind: "kubernetes".to_string(),
            name: call.source_id.clone(),
            path: source_path,
        },
        target: call.target.clone(),
        timestamp: Some(Utc::now().to_rfc3339()),
        confidence: 0.8,
        data,
        references: Vec::new(),
    };
    Ok((ToolResultStatus::Succeeded, vec![evidence], None))
}

fn skipped_result(message: &str) -> (ToolResultStatus, Vec<Evidence>, Option<String>) {
    (
        ToolResultStatus::Skipped,
        Vec::new(),
        Some(message.to_string()),
    )
}

fn failed_result(message: &str) -> (ToolResultStatus, Vec<Evidence>, Option<String>) {
    (
        ToolResultStatus::Failed,
        Vec::new(),
        Some(message.to_string()),
    )
}

async fn fetch_alertmanager_alerts(
    base_url: &str,
    call: &ToolCall,
    bearer_token_env: Option<&str>,
) -> Result<Value, String> {
    let mut query = vec![
        ("active".to_string(), "true".to_string()),
        ("silenced".to_string(), "true".to_string()),
        ("inhibited".to_string(), "true".to_string()),
    ];
    if let Some(matcher) = call.inputs.get("matcher").and_then(Value::as_str) {
        query.push(("filter".to_string(), matcher.to_string()));
    }
    http_get_json(
        &format!("{}/api/v2/alerts", base_url.trim_end_matches('/')),
        &query,
        bearer_token_env,
    )
    .await
}

async fn fetch_prometheus_query_range(
    base_url: &str,
    call: &ToolCall,
    query_text: &str,
    bearer_token_env: Option<&str>,
) -> Result<Value, String> {
    let (start, end) = query_window(call.since.as_deref());
    let step = call
        .inputs
        .get("step")
        .and_then(Value::as_str)
        .unwrap_or("60s");
    let query = vec![
        ("query".to_string(), query_text.to_string()),
        ("start".to_string(), start.to_rfc3339()),
        ("end".to_string(), end.to_rfc3339()),
        ("step".to_string(), step.to_string()),
    ];
    http_get_json(
        &format!("{}/api/v1/query_range", base_url.trim_end_matches('/')),
        &query,
        bearer_token_env,
    )
    .await
}

async fn fetch_github_commits(
    api_url: Option<&str>,
    repo: &str,
    call: &ToolCall,
    bearer_token_env: Option<&str>,
) -> Result<Value, String> {
    let (start, _end) = query_window(call.since.as_deref());
    let base_url = api_url
        .unwrap_or("https://api.github.com")
        .trim_end_matches('/');
    let query = vec![
        ("since".to_string(), start.to_rfc3339()),
        ("per_page".to_string(), "20".to_string()),
    ];
    let url = format!("{base_url}/repos/{repo}/commits");
    let client = reqwest::Client::new();
    let mut request = client
        .get(&url)
        .header("accept", "application/vnd.github+json")
        .header("user-agent", "vigil")
        .header("x-github-api-version", "2026-03-10")
        .query(&query);
    request = apply_bearer_auth(request, bearer_token_env)?;
    send_json_request(request).await
}

async fn fetch_http_check(url: &str, bearer_token_env: Option<&str>) -> Result<Value, String> {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .map_err(|err| format!("HTTP client could not be built: {err}"))?;
    let request = apply_bearer_auth(
        client.get(url).header("user-agent", "vigil"),
        bearer_token_env,
    )?;
    let response = request
        .send()
        .await
        .map_err(|err| format!("HTTP check failed: {err}"))?;
    let status = response.status();
    let final_url = response.url().to_string();
    let headers = response
        .headers()
        .iter()
        .filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|value| (name.as_str().to_string(), Value::String(value.to_string())))
        })
        .collect::<serde_json::Map<_, _>>();
    let body = response
        .text()
        .await
        .map_err(|err| format!("HTTP response body could not be read: {err}"))?;
    Ok(json!({
        "url": final_url,
        "status": status.as_u16(),
        "success": status.is_success(),
        "headers": headers,
        "body_preview": truncate_text(&body, 2048)
    }))
}

async fn fetch_loki_query_range(
    base_url: &str,
    call: &ToolCall,
    query_text: &str,
    bearer_token_env: Option<&str>,
) -> Result<Value, String> {
    let query = vec![
        ("query".to_string(), query_text.to_string()),
        (
            "since".to_string(),
            call.since.clone().unwrap_or_else(|| "30m".to_string()),
        ),
        (
            "limit".to_string(),
            call.inputs
                .get("limit")
                .and_then(Value::as_u64)
                .unwrap_or(50)
                .to_string(),
        ),
        ("direction".to_string(), "backward".to_string()),
    ];
    http_get_json(
        &format!("{}/loki/api/v1/query_range", base_url.trim_end_matches('/')),
        &query,
        bearer_token_env,
    )
    .await
}

async fn fetch_grafana_annotations(
    base_url: &str,
    call: &ToolCall,
    bearer_token_env: Option<&str>,
) -> Result<Value, String> {
    let (start, end) = query_window(call.since.as_deref());
    let mut query = vec![
        ("from".to_string(), start.timestamp_millis().to_string()),
        ("to".to_string(), end.timestamp_millis().to_string()),
    ];
    if let Some(tags) = call.inputs.get("tags").and_then(Value::as_array) {
        for tag in tags.iter().filter_map(Value::as_str) {
            query.push(("tags".to_string(), tag.to_string()));
        }
    }
    http_get_json(
        &format!("{}/api/annotations", base_url.trim_end_matches('/')),
        &query,
        bearer_token_env,
    )
    .await
}

async fn fetch_kubernetes_events(
    base_url: &str,
    namespace: Option<&str>,
    call: &ToolCall,
    bearer_token_env: Option<&str>,
) -> Result<Value, String> {
    let path = namespace
        .map(|namespace| format!("/apis/events.k8s.io/v1/namespaces/{namespace}/events"))
        .unwrap_or_else(|| "/apis/events.k8s.io/v1/events".to_string());
    let mut query = vec![(
        "limit".to_string(),
        call.inputs
            .get("limit")
            .and_then(Value::as_u64)
            .unwrap_or(50)
            .to_string(),
    )];
    if let Some(selector) = call.inputs.get("field_selector").and_then(Value::as_str) {
        query.push(("fieldSelector".to_string(), selector.to_string()));
    }
    http_get_json(
        &format!("{}{}", base_url.trim_end_matches('/'), path),
        &query,
        bearer_token_env,
    )
    .await
}

async fn http_get_json(
    url: &str,
    query: &[(String, String)],
    bearer_token_env: Option<&str>,
) -> Result<Value, String> {
    let client = reqwest::Client::new();
    let request = client.get(url).header("user-agent", "vigil").query(query);
    let request = apply_bearer_auth(request, bearer_token_env)?;
    send_json_request(request).await
}

fn apply_bearer_auth(
    request: reqwest::RequestBuilder,
    bearer_token_env: Option<&str>,
) -> Result<reqwest::RequestBuilder, String> {
    match bearer_token_env {
        Some(env_name) => {
            let token = env::var(env_name)
                .map_err(|_| format!("bearer token env var '{env_name}' is not set"))?;
            Ok(request.bearer_auth(token))
        }
        None => Ok(request),
    }
}

async fn send_json_request(request: reqwest::RequestBuilder) -> Result<Value, String> {
    let response = request
        .send()
        .await
        .map_err(|err| format!("read-only HTTP request failed: {err}"))?;
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|err| format!("read-only HTTP response body could not be read: {err}"))?;
    if !status.is_success() {
        return Err(format!(
            "read-only HTTP request returned HTTP {}: {}",
            status.as_u16(),
            truncate_text(&body, 500)
        ));
    }
    serde_json::from_str(&body)
        .map_err(|err| format!("read-only HTTP response was not JSON: {err}"))
}

fn query_window(since: Option<&str>) -> (chrono::DateTime<Utc>, chrono::DateTime<Utc>) {
    let end = Utc::now();
    let duration = since
        .and_then(parse_duration)
        .unwrap_or_else(|| ChronoDuration::minutes(30));
    (end - duration, end)
}

fn parse_duration(value: &str) -> Option<ChronoDuration> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    let (number, unit) = trimmed.split_at(trimmed.len().saturating_sub(1));
    let amount = number.parse::<i64>().ok()?;
    match unit {
        "s" => Some(ChronoDuration::seconds(amount)),
        "m" => Some(ChronoDuration::minutes(amount)),
        "h" => Some(ChronoDuration::hours(amount)),
        "d" => Some(ChronoDuration::days(amount)),
        _ => None,
    }
}

fn resolve_dns_host(host: &str) -> Result<Value, String> {
    let addresses = (host, 0)
        .to_socket_addrs()
        .map_err(|err| format!("DNS lookup for '{host}' failed: {err}"))?
        .map(|address| Value::String(address.ip().to_string()))
        .collect::<Vec<_>>();
    Ok(json!({
        "host": host,
        "addresses": addresses
    }))
}

fn truncate_text(value: &str, limit: usize) -> String {
    if value.len() <= limit {
        value.to_string()
    } else {
        format!("{}...", &value[..limit])
    }
}

fn parse_alertmanager_alerts(value: Value) -> Result<Vec<Alert>, CoreError> {
    let alerts_value = value
        .get("alerts")
        .or_else(|| value.get("data"))
        .cloned()
        .unwrap_or(value);
    let Some(alerts) = alerts_value.as_array() else {
        return Err(CoreError::Validation {
            kind: "alertmanager response",
            path: "live".to_string(),
            errors: "expected an array of alerts".to_string(),
        });
    };

    let parsed = alerts
        .iter()
        .enumerate()
        .map(|(index, alert)| alertmanager_value_to_alert(index, alert))
        .collect::<Result<Vec<_>, _>>()?;
    validate_alerts_fixture(&parsed, Path::new("live"))?;
    Ok(parsed)
}

fn alertmanager_value_to_alert(index: usize, value: &Value) -> Result<Alert, CoreError> {
    let labels = string_map(value.get("labels"));
    let annotations = value
        .get("annotations")
        .and_then(Value::as_object)
        .map(|map| {
            map.iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();
    let name = labels
        .get("alertname")
        .cloned()
        .unwrap_or_else(|| format!("alert-{index}"));
    let summary = annotations
        .get("summary")
        .and_then(Value::as_str)
        .or_else(|| annotations.get("description").and_then(Value::as_str))
        .unwrap_or(&name)
        .to_string();
    let severity = labels
        .get("severity")
        .cloned()
        .unwrap_or_else(|| "unknown".to_string());
    let status = value
        .pointer("/status/state")
        .and_then(Value::as_str)
        .or_else(|| value.get("status").and_then(Value::as_str))
        .unwrap_or("unknown")
        .to_string();
    let target = labels
        .get("target")
        .or_else(|| labels.get("service"))
        .map(|value| {
            if value.contains(':') {
                value.clone()
            } else {
                format!("service:{value}")
            }
        });

    Ok(Alert {
        id: value
            .get("fingerprint")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| format!("{}-{index}", slug_id(&name))),
        name,
        severity,
        status,
        summary,
        description: annotations
            .get("description")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        target,
        started_at: value
            .get("startsAt")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        ended_at: value
            .get("endsAt")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        labels,
        annotations,
        source: Some("alertmanager".to_string()),
    })
}

fn string_map(value: Option<&Value>) -> BTreeMap<String, String> {
    value
        .and_then(Value::as_object)
        .map(|map| {
            map.iter()
                .filter_map(|(key, value)| {
                    value.as_str().map(|value| (key.clone(), value.to_string()))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn load_alerts_fixture(path: &Path) -> Result<Vec<Alert>, CoreError> {
    let value: Value = parse_document("alertmanager fixture", path)?;
    let alerts_value = value
        .get("alerts")
        .cloned()
        .unwrap_or_else(|| value.clone());
    if alerts_value.is_array() {
        let alerts: Vec<Alert> =
            serde_json::from_value(alerts_value).map_err(|source| CoreError::ParseJson {
                kind: "alertmanager fixture",
                path: path.display().to_string(),
                source,
            })?;
        validate_alerts_fixture(&alerts, path)?;
        Ok(alerts)
    } else {
        let alert: Alert =
            serde_json::from_value(alerts_value).map_err(|source| CoreError::ParseJson {
                kind: "alertmanager fixture",
                path: path.display().to_string(),
                source,
            })?;
        validate_alerts_fixture(std::slice::from_ref(&alert), path)?;
        Ok(vec![alert])
    }
}

fn validate_alerts_fixture(alerts: &[Alert], path: &Path) -> Result<(), CoreError> {
    let errors = alerts.iter().flat_map(Alert::validate).collect::<Vec<_>>();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(CoreError::Validation {
            kind: "alertmanager fixture",
            path: display_path(path),
            errors: errors.join("; "),
        })
    }
}

fn alert_matches(alert: &Alert, matcher: Option<&str>, target: Option<&str>) -> bool {
    let matcher_matches = matcher.is_some_and(|matcher| {
        let matcher = matcher.trim();
        if let Some((key, value)) = matcher.split_once('=') {
            (key == "alertname" && alert.name == value)
                || alert.labels.get(key).is_some_and(|label| label == value)
        } else {
            alert.name == matcher
                || alert.id == matcher
                || alert.target.as_deref() == Some(matcher)
                || alert.labels.values().any(|value| value == matcher)
        }
    });
    let target_matches = target.is_some_and(|target| alert.target.as_deref() == Some(target));
    if matcher.is_none() && target.is_none() {
        true
    } else {
        matcher_matches || target_matches
    }
}

fn alert_to_evidence(
    alert: &Alert,
    source_name: &str,
    path: Option<&Path>,
) -> Result<Evidence, CoreError> {
    Ok(Evidence {
        id: format!("alertmanager:{}", alert.id),
        kind: EvidenceKind::Alert,
        summary: alert.summary.clone(),
        source: EvidenceSource {
            kind: "alertmanager".to_string(),
            name: source_name.to_string(),
            path: path.map(display_path),
        },
        target: alert.target.clone(),
        timestamp: alert.started_at.clone(),
        confidence: 0.9,
        data: serde_json::to_value(alert).map_err(|err| CoreError::Redaction(err.to_string()))?,
        references: Vec::new(),
    })
}

fn tool_call_dedupe_key(call: &ToolCall) -> String {
    let inputs = serde_json::to_string(&call.inputs).unwrap_or_default();
    format!(
        "{}|{}|{}|{}",
        call.capability_id,
        call.source_id,
        call.target.as_deref().unwrap_or_default(),
        inputs
    )
}

pub fn load_alert(path: &Path) -> Result<Alert, CoreError> {
    let alert: Alert = parse_document("alert", path)?;
    let errors = alert.validate();
    if errors.is_empty() {
        Ok(alert)
    } else {
        Err(CoreError::Validation {
            kind: "alert",
            path: path.display().to_string(),
            errors: errors.join("; "),
        })
    }
}

pub fn load_inventory(path: &Path) -> Result<Inventory, CoreError> {
    let inventory: Inventory = parse_document("inventory", path)?;
    let errors = inventory.validate();
    if errors.is_empty() {
        Ok(inventory)
    } else {
        Err(CoreError::Validation {
            kind: "inventory",
            path: path.display().to_string(),
            errors: errors.join("; "),
        })
    }
}

pub fn load_runbook(path: &Path) -> Result<Runbook, CoreError> {
    let runbook: Runbook = parse_document("runbook", path)?;
    let errors = runbook.validate();
    if errors.is_empty() {
        Ok(runbook)
    } else {
        Err(CoreError::Validation {
            kind: "runbook",
            path: path.display().to_string(),
            errors: errors.join("; "),
        })
    }
}

pub fn load_runbooks(
    explicit_paths: &[PathBuf],
    runbook_dir: Option<&Path>,
) -> Result<Vec<Runbook>, CoreError> {
    let mut paths = explicit_paths.to_vec();
    if let Some(dir) = runbook_dir {
        if !dir.is_dir() {
            return Err(CoreError::RunbookDirNotDirectory {
                path: dir.display().to_string(),
            });
        }
        let entries = fs::read_dir(dir).map_err(|source| CoreError::ReadRunbookDir {
            path: dir.display().to_string(),
            source,
        })?;
        for entry in entries {
            let entry = entry.map_err(|source| CoreError::ReadRunbookDir {
                path: dir.display().to_string(),
                source,
            })?;
            let path = entry.path();
            if is_supported_document(&path) {
                paths.push(path);
            }
        }
    }

    paths.sort();
    paths.dedup();

    paths
        .iter()
        .map(|path| load_runbook(path))
        .collect::<Result<Vec<_>, _>>()
}

pub fn load_case_evidence(path: &Path) -> Result<Evidence, CoreError> {
    let evidence: Evidence = parse_document("evidence", path)?;
    validate_evidence(&evidence, path)?;
    Ok(evidence)
}

fn load_case_evidence_dir(evidence_dir: &Path) -> Result<Vec<Evidence>, CoreError> {
    supported_document_paths("evidence", evidence_dir)?
        .iter()
        .map(|path| load_case_evidence(path))
        .collect()
}

fn supported_document_paths(kind: &'static str, dir: &Path) -> Result<Vec<PathBuf>, CoreError> {
    let entries = fs::read_dir(dir).map_err(|source| CoreError::ReadDirectory {
        kind,
        path: dir.display().to_string(),
        source,
    })?;
    let mut paths = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|source| CoreError::ReadDirectory {
            kind,
            path: dir.display().to_string(),
            source,
        })?;
        let path = entry.path();
        if is_supported_document(&path) {
            paths.push(path);
        }
    }
    paths.sort();
    Ok(paths)
}

fn case_manifest_path(case_dir: &Path) -> PathBuf {
    case_dir.join("vigil.yaml")
}

fn case_id_from_dir(case_dir: &Path) -> String {
    case_dir
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("case")
        .to_string()
}

fn case_title_from_id(id: &str) -> String {
    let words = id
        .split(['-', '_'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>();
    format!("{} investigation", words.join(" "))
}

fn create_case_subdir(path: &Path) -> Result<(), CoreError> {
    fs::create_dir_all(path).map_err(|source| CoreError::CreateCaseDir {
        path: path.display().to_string(),
        source,
    })
}

fn required_case_subdir(case_dir: &Path, subdir: &'static str) -> Result<PathBuf, CoreError> {
    let path = case_dir.join(subdir);
    if path.is_dir() {
        Ok(path)
    } else {
        Err(CoreError::MissingCaseSubdir {
            path: case_dir.display().to_string(),
            subdir,
        })
    }
}

fn validate_case_manifest(manifest: &CaseManifest, path: &Path) -> Result<(), CoreError> {
    let errors = manifest.validate();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(CoreError::Validation {
            kind: "case manifest",
            path: path.display().to_string(),
            errors: errors.join("; "),
        })
    }
}

fn validate_evidence(evidence: &Evidence, path: &Path) -> Result<(), CoreError> {
    let errors = evidence.validate();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(CoreError::Validation {
            kind: "evidence",
            path: path.display().to_string(),
            errors: errors.join("; "),
        })
    }
}

fn write_yaml_file<T>(path: &Path, value: &T, kind: &'static str) -> Result<(), CoreError>
where
    T: serde::Serialize,
{
    let yaml =
        serde_yaml::to_string(value).map_err(|source| CoreError::SerializeYaml { kind, source })?;
    fs::write(path, yaml).map_err(|source| CoreError::WriteCaseFile {
        path: path.display().to_string(),
        source,
    })
}

fn next_case_evidence_path(evidence_dir: &Path, kind: &EvidenceKind) -> Result<PathBuf, CoreError> {
    let prefix = kind.file_prefix();
    for index in 1..=9999 {
        let path = evidence_dir.join(format!("{prefix}-{index:03}.yaml"));
        if !path.exists() {
            return Ok(path);
        }
    }
    Err(CoreError::Validation {
        kind: "evidence",
        path: evidence_dir.display().to_string(),
        errors: format!("could not allocate a new {prefix} evidence file name"),
    })
}

fn build_case_evidence_item(
    manifest: &CaseManifest,
    kind: EvidenceKind,
    id: &str,
    summary: &str,
    source: &str,
    url: Option<&str>,
    file: Option<&Path>,
) -> Result<Evidence, CoreError> {
    let mut references = Vec::new();
    let mut data = serde_json::Map::new();
    data.insert("source".to_string(), Value::String(source.to_string()));

    if let Some(url) = url {
        data.insert("url".to_string(), Value::String(url.to_string()));
        references.push(SourceReference {
            title: Some(source.to_string()),
            url: Some(url.to_string()),
            path: None,
        });
    }

    if let Some(file) = file {
        let content = read_to_string("evidence source", file)?;
        data.insert(
            "file".to_string(),
            json!({
                "path": file.display().to_string(),
                "content": content
            }),
        );
        references.push(SourceReference {
            title: Some(source.to_string()),
            url: None,
            path: Some(file.display().to_string()),
        });
    }

    Ok(Evidence {
        id: id.to_string(),
        kind,
        summary: summary.to_string(),
        source: EvidenceSource {
            kind: "case_input".to_string(),
            name: source.to_string(),
            path: None,
        },
        target: Some(manifest.target.clone()),
        timestamp: Some(Utc::now().to_rfc3339()),
        confidence: 1.0,
        data: Value::Object(data),
        references,
    })
}

fn target_from_case_manifest(manifest: &CaseManifest) -> Target {
    let (kind, name) = manifest
        .target
        .split_once(':')
        .map(|(kind, name)| (target_kind_from_label(kind), name.to_string()))
        .unwrap_or((TargetKind::Unknown, manifest.target.clone()));

    Target {
        id: manifest.target.clone(),
        kind,
        name,
        environment: None,
        service: None,
        host: None,
        labels: BTreeMap::new(),
        criticality: Some(manifest.severity.clone()),
        metadata: BTreeMap::from([
            ("case_id".to_string(), Value::String(manifest.id.clone())),
            (
                "case_status".to_string(),
                Value::String(manifest.status.clone()),
            ),
        ]),
    }
}

fn target_kind_from_label(kind: &str) -> TargetKind {
    match kind {
        "service" => TargetKind::Service,
        "host" => TargetKind::Host,
        "component" => TargetKind::Component,
        "endpoint" => TargetKind::Endpoint,
        _ => TargetKind::Unknown,
    }
}

fn alert_from_case_manifest(manifest: &CaseManifest) -> Alert {
    Alert {
        id: manifest.id.clone(),
        name: manifest.title.clone(),
        severity: manifest.severity.clone(),
        status: manifest.status.clone(),
        summary: manifest.summary.clone(),
        description: None,
        target: Some(manifest.target.clone()),
        started_at: Some(manifest.created_at.clone()),
        ended_at: None,
        labels: BTreeMap::new(),
        annotations: BTreeMap::from([("case_id".to_string(), Value::String(manifest.id.clone()))]),
        source: Some("case_manifest".to_string()),
    }
}

fn build_case_evidence(
    manifest: &CaseManifest,
    alert: &Alert,
    target: &Target,
    supplied_evidence: Vec<Evidence>,
    runbooks: &[Runbook],
) -> Result<Vec<Evidence>, CoreError> {
    let mut evidence = vec![
        Evidence {
            id: format!("case:{}", manifest.id),
            kind: EvidenceKind::Alert,
            summary: manifest.summary.clone(),
            source: EvidenceSource {
                kind: "case_manifest".to_string(),
                name: manifest.id.clone(),
                path: Some("vigil.yaml".to_string()),
            },
            target: Some(manifest.target.clone()),
            timestamp: Some(manifest.created_at.clone()),
            confidence: 1.0,
            data: serde_json::to_value(alert)
                .map_err(|err| CoreError::Redaction(err.to_string()))?,
            references: Vec::new(),
        },
        Evidence {
            id: format!("target:{}", target.id),
            kind: EvidenceKind::Inventory,
            summary: format!(
                "Case manifest identifies '{}' as the investigation target.",
                target.id
            ),
            source: EvidenceSource {
                kind: "case_manifest".to_string(),
                name: manifest.id.clone(),
                path: Some("vigil.yaml".to_string()),
            },
            target: Some(target.id.clone()),
            timestamp: Some(manifest.created_at.clone()),
            confidence: 1.0,
            data: serde_json::to_value(target)
                .map_err(|err| CoreError::Redaction(err.to_string()))?,
            references: Vec::new(),
        },
    ];
    evidence.extend(supplied_evidence);

    for runbook in matching_runbooks(runbooks, std::slice::from_ref(target)) {
        evidence.push(Evidence {
            id: format!("runbook:{}", runbook.id),
            kind: EvidenceKind::Runbook,
            summary: format!(
                "Runbook '{}' supplies {} read-only check(s).",
                runbook.title,
                runbook.checks.len()
            ),
            source: EvidenceSource {
                kind: "case_runbook".to_string(),
                name: runbook.id.clone(),
                path: Some(format!("runbooks/{}.yaml", runbook.id)),
            },
            target: Some(target.id.clone()),
            timestamp: None,
            confidence: 0.9,
            data: serde_json::to_value(&runbook)
                .map_err(|err| CoreError::Redaction(err.to_string()))?,
            references: runbook.references.clone(),
        });
    }

    for item in &evidence {
        validate_evidence(item, Path::new("generated"))?;
    }

    Ok(evidence)
}

pub fn redact_evidence_packet(packet: EvidencePacket) -> Result<EvidencePacket, CoreError> {
    let mut value =
        serde_json::to_value(&packet).map_err(|err| CoreError::Redaction(err.to_string()))?;
    let mut masked_values = 0;
    redact_value(&mut value, None, &mut masked_values);

    let mut packet: EvidencePacket =
        serde_json::from_value(value).map_err(|err| CoreError::Redaction(err.to_string()))?;
    let mut warnings = vec![
        "Basic redaction was applied before any LLM request; secret detection is best-effort."
            .to_string(),
    ];
    if masked_values > 0 {
        warnings.push(format!(
            "Masked {masked_values} potentially sensitive value(s)."
        ));
    }
    packet.redaction = RedactionReport {
        applied: true,
        masked_values,
        warnings,
    };
    Ok(packet)
}

fn parse_document<T>(kind: &'static str, path: &Path) -> Result<T, CoreError>
where
    T: serde::de::DeserializeOwned,
{
    let text = read_to_string(kind, path)?;
    if path
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
    {
        serde_json::from_str(&text).map_err(|source| CoreError::ParseJson {
            kind,
            path: path.display().to_string(),
            source,
        })
    } else {
        serde_yaml::from_str(&text).map_err(|source| CoreError::ParseYaml {
            kind,
            path: path.display().to_string(),
            source,
        })
    }
}

fn read_to_string(kind: &'static str, path: &Path) -> Result<String, CoreError> {
    fs::read_to_string(path).map_err(|source| CoreError::ReadInput {
        kind,
        path: path.display().to_string(),
        source,
    })
}

fn is_supported_document(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "yaml" | "yml" | "json"
            )
        })
}

fn resolve_targets(
    alert: Option<&Alert>,
    inventory: &Inventory,
    requested: Option<&str>,
) -> Vec<Target> {
    let mut candidates = BTreeMap::new();
    for target in inventory_targets(inventory) {
        candidates.insert(target.id.clone(), target);
    }

    let mut selectors = Vec::new();
    if let Some(requested) = requested {
        selectors.push(requested.to_string());
    }
    if let Some(alert_target) = alert.and_then(|alert| alert.target.as_ref()) {
        selectors.push(alert_target.clone());
    }

    let mut resolved = Vec::new();
    for selector in selectors {
        if let Some(target) = find_target(&candidates, &selector) {
            push_unique_target(&mut resolved, target);
        } else {
            push_unique_target(&mut resolved, unknown_target(&selector));
        }
    }

    if resolved.is_empty() {
        if let Some(target) = candidates.values().next() {
            resolved.push(target.clone());
        }
    }

    resolved
}

fn inventory_targets(inventory: &Inventory) -> Vec<Target> {
    let mut targets = inventory.targets.clone();
    for service in &inventory.services {
        targets.push(Target {
            id: service.id.clone(),
            kind: TargetKind::Service,
            name: service.name.clone(),
            environment: None,
            service: Some(service.name.clone()),
            host: None,
            labels: service.labels.clone(),
            criticality: service
                .metadata
                .get("criticality")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            metadata: service.metadata.clone(),
        });
    }
    for host in &inventory.hosts {
        targets.push(Target {
            id: host.id.clone(),
            kind: TargetKind::Host,
            name: host.name.clone(),
            environment: host.environment.clone(),
            service: None,
            host: Some(host.name.clone()),
            labels: host.labels.clone(),
            criticality: host
                .metadata
                .get("criticality")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            metadata: host.metadata.clone(),
        });
    }
    targets
}

fn find_target(targets: &BTreeMap<String, Target>, selector: &str) -> Option<Target> {
    if let Some(target) = targets.get(selector) {
        return Some(target.clone());
    }

    let (kind, name) = selector
        .split_once(':')
        .map(|(kind, name)| (Some(kind), name))
        .unwrap_or((None, selector));

    targets.values().find_map(|target| {
        let kind_matches = match kind {
            Some(kind) => target_kind_label(&target.kind) == kind,
            None => true,
        };
        let name_matches = target.name == name
            || target.service.as_deref() == Some(name)
            || target.host.as_deref() == Some(name)
            || target.id == name;
        if kind_matches && name_matches {
            Some(target.clone())
        } else {
            None
        }
    })
}

fn target_kind_label(kind: &TargetKind) -> &'static str {
    match kind {
        TargetKind::Service => "service",
        TargetKind::Host => "host",
        TargetKind::Component => "component",
        TargetKind::Endpoint => "endpoint",
        TargetKind::Unknown => "unknown",
    }
}

fn unknown_target(selector: &str) -> Target {
    Target {
        id: selector.to_string(),
        kind: TargetKind::Unknown,
        name: selector.to_string(),
        environment: None,
        service: None,
        host: None,
        labels: BTreeMap::new(),
        criticality: None,
        metadata: BTreeMap::from([(
            "resolution".to_string(),
            Value::String("not found in inventory".to_string()),
        )]),
    }
}

fn push_unique_target(targets: &mut Vec<Target>, target: Target) {
    if !targets.iter().any(|existing| existing.id == target.id) {
        targets.push(target);
    }
}

fn build_evidence(
    alert: Option<&Alert>,
    inventory: &Inventory,
    targets: &[Target],
    runbooks: &[Runbook],
) -> Result<Vec<Evidence>, CoreError> {
    let mut evidence = Vec::new();

    if let Some(alert) = alert {
        evidence.push(Evidence {
            id: format!("alert:{}", alert.id),
            kind: EvidenceKind::Alert,
            summary: alert.summary.clone(),
            source: EvidenceSource {
                kind: "input_file".to_string(),
                name: alert.source.clone().unwrap_or_else(|| "alert".to_string()),
                path: None,
            },
            target: alert.target.clone(),
            timestamp: alert.started_at.clone(),
            confidence: 1.0,
            data: serde_json::to_value(alert)
                .map_err(|err| CoreError::Redaction(err.to_string()))?,
            references: Vec::new(),
        });
    }

    for target in targets {
        evidence.push(Evidence {
            id: format!("inventory:{}", target.id),
            kind: EvidenceKind::Inventory,
            summary: format!(
                "Inventory identifies '{}' as a {:?} target.",
                target.name, target.kind
            ),
            source: EvidenceSource {
                kind: "input_file".to_string(),
                name: "inventory".to_string(),
                path: None,
            },
            target: Some(target.id.clone()),
            timestamp: None,
            confidence: 1.0,
            data: target_to_inventory_context(target, inventory),
            references: Vec::new(),
        });
    }

    for runbook in matching_runbooks(runbooks, targets) {
        evidence.push(Evidence {
            id: format!("runbook:{}", runbook.id),
            kind: EvidenceKind::Runbook,
            summary: format!(
                "Runbook '{}' supplies {} read-only check(s).",
                runbook.title,
                runbook.checks.len()
            ),
            source: EvidenceSource {
                kind: "input_file".to_string(),
                name: "runbook".to_string(),
                path: None,
            },
            target: targets.first().map(|target| target.id.clone()),
            timestamp: None,
            confidence: 0.9,
            data: serde_json::to_value(&runbook)
                .map_err(|err| CoreError::Redaction(err.to_string()))?,
            references: runbook.references.clone(),
        });
    }

    for item in &evidence {
        let errors = item.validate();
        if !errors.is_empty() {
            return Err(CoreError::Validation {
                kind: "evidence",
                path: "generated".to_string(),
                errors: errors.join("; "),
            });
        }
    }

    Ok(evidence)
}

fn target_to_inventory_context(target: &Target, inventory: &Inventory) -> Value {
    let dependencies = inventory
        .dependencies
        .iter()
        .filter(|dependency| dependency.from == target.id || dependency.to == target.id)
        .collect::<Vec<_>>();
    json!({
        "target": target,
        "dependencies": dependencies
    })
}

fn matching_runbooks(runbooks: &[Runbook], targets: &[Target]) -> Vec<Runbook> {
    runbooks
        .iter()
        .filter(|runbook| runbook_applies(runbook, targets))
        .cloned()
        .collect()
}

fn runbook_applies(runbook: &Runbook, targets: &[Target]) -> bool {
    if runbook.applies_to.is_empty() || runbook.applies_to.iter().any(|item| item == "*") {
        return true;
    }

    targets.iter().any(|target| {
        runbook.applies_to.iter().any(|selector| {
            selector == &target.id
                || selector == &target.name
                || selector == target_kind_label(&target.kind)
                || target.service.as_ref() == Some(selector)
                || target.host.as_ref() == Some(selector)
        })
    })
}

fn investigation_question(
    alert: Option<&Alert>,
    requested: Option<&str>,
    targets: &[Target],
) -> String {
    if let Some(alert) = alert {
        return format!(
            "Investigate alert '{}' affecting {}.",
            alert.name,
            alert.target.as_deref().unwrap_or("an unresolved target")
        );
    }
    if let Some(requested) = requested {
        return format!("Investigate target '{requested}'.");
    }
    if let Some(target) = targets.first() {
        return format!("Investigate target '{}'.", target.id);
    }
    "Investigate the supplied operational evidence.".to_string()
}

fn investigation_mode(request: &InvestigationRequest) -> &'static str {
    if request.no_llm {
        "no_llm"
    } else if request.dry_run {
        "dry_run"
    } else {
        "llm"
    }
}

fn deterministic_reasoning(packet: &EvidencePacket) -> Result<ReasoningResult, CoreError> {
    let target_id = packet
        .targets
        .first()
        .map(|target| target.id.clone())
        .unwrap_or_else(|| "unknown".to_string());
    let summary = if let Some(alert) = packet.alerts.first() {
        format!(
            "Deterministic investigation draft for alert '{}' on {}.",
            alert.name,
            alert.target.as_deref().unwrap_or(&target_id)
        )
    } else {
        format!("Deterministic investigation draft for target '{target_id}'.")
    };

    let mut recommended_checks = vec![
        RecommendedCheck {
            id: "check-service-health".to_string(),
            title: "Review service health dashboards".to_string(),
            description:
                "Compare error rate, latency, saturation, and availability signals during the investigation window."
                    .to_string(),
            target: Some(target_id.clone()),
            reason: "Read-only telemetry can confirm whether the symptom is isolated or broad."
                .to_string(),
            read_only: true,
            source: "deterministic-no-llm".to_string(),
            related_evidence_ids: packet.evidence.iter().map(|item| item.id.clone()).collect(),
        },
        RecommendedCheck {
            id: "check-recent-changes".to_string(),
            title: "Review recent change records".to_string(),
            description:
                "Compare deployments, configuration changes, and dependency changes with the alert start time."
                    .to_string(),
            target: Some(target_id.clone()),
            reason: "Recent change evidence is not present in the supplied inputs.".to_string(),
            read_only: true,
            source: "deterministic-no-llm".to_string(),
            related_evidence_ids: packet.evidence.iter().map(|item| item.id.clone()).collect(),
        },
    ];

    for runbook in &packet.runbooks {
        for check in &runbook.checks {
            recommended_checks.push(RecommendedCheck {
                id: format!("runbook-{}-{}", runbook.id, check.id),
                title: check.title.clone(),
                description: check.description.clone(),
                target: Some(target_id.clone()),
                reason: format!("Recommended by runbook '{}'.", runbook.title),
                read_only: check.read_only,
                source: format!("runbook:{}", runbook.id),
                related_evidence_ids: vec![format!("runbook:{}", runbook.id)],
            });
        }
    }

    let result = ReasoningResult {
        summary,
        hypotheses: vec![Hypothesis {
            id: "hyp-alert-reflects-real-impact".to_string(),
            title: "The supplied symptom reflects real user or service impact".to_string(),
            description:
                "The alert and inventory evidence are sufficient to begin investigation, but more telemetry is needed before choosing a root cause."
                    .to_string(),
            confidence: 0.4,
            supporting_evidence_ids: packet.evidence.iter().map(|item| item.id.clone()).collect(),
            contradicting_evidence_ids: Vec::new(),
            missing_checks: vec![
                "missing-recent-changes".to_string(),
                "missing-correlated-telemetry".to_string(),
            ],
            risk_if_wrong: "The alert may be noisy or scoped differently than the supplied inventory."
                .to_string(),
        }],
        missing_checks: vec![
            MissingCheck {
                id: "missing-recent-changes".to_string(),
                title: "Recent changes".to_string(),
                description:
                    "No deployment, configuration, dependency, or incident timeline evidence was supplied."
                        .to_string(),
                target: Some(target_id.clone()),
                reason: "Recent changes often explain sudden operational symptoms.".to_string(),
                related_evidence_ids: packet.evidence.iter().map(|item| item.id.clone()).collect(),
            },
            MissingCheck {
                id: "missing-correlated-telemetry".to_string(),
                title: "Correlated telemetry".to_string(),
                description:
                    "Metrics, logs, and traces around the alert window were not supplied.".to_string(),
                target: Some(target_id),
                reason: "Correlated read-only evidence is needed before treating any hypothesis as likely."
                    .to_string(),
                related_evidence_ids: packet.evidence.iter().map(|item| item.id.clone()).collect(),
            },
        ],
        recommended_checks,
        risk_notes: vec!["Deterministic mode does not use an LLM and may miss context.".to_string()],
        operator_notes: vec!["Vigil did not execute any checks or mutate production.".to_string()],
        confidence_notes: vec![
            "Confidence is intentionally conservative without telemetry and change data.".to_string(),
        ],
    };

    validate_reasoning_result(&result)
        .map_err(|err| CoreError::DeterministicReasoning(err.to_string()))?;
    Ok(result)
}

fn provider_response_parts(
    response: ProviderResponse,
) -> (ReasoningResult, Option<LlmExchangeMetadata>) {
    (response.reasoning, Some(response.metadata))
}

fn build_brief(
    packet: &EvidencePacket,
    reasoning: &ReasoningResult,
    warnings: &[String],
) -> EvidenceBrief {
    let title_target = packet
        .targets
        .first()
        .map(|target| target.name.as_str())
        .unwrap_or("investigation");
    let mut all_warnings = warnings.to_vec();
    all_warnings
        .push("Recommended checks are advisory and were not executed by Vigil.".to_string());

    EvidenceBrief {
        title: format!("Investigation Brief: {title_target}"),
        summary: reasoning.summary.clone(),
        targets: packet.targets.clone(),
        evidence: packet.evidence.clone(),
        hypotheses: reasoning.hypotheses.clone(),
        missing_checks: reasoning.missing_checks.clone(),
        recommended_checks: reasoning.recommended_checks.clone(),
        risk_notes: reasoning.risk_notes.clone(),
        references: collect_references(packet),
        warnings: dedup_strings(all_warnings),
    }
}

fn collect_references(packet: &EvidencePacket) -> Vec<SourceReference> {
    let mut references = Vec::new();
    let mut seen = BTreeSet::new();
    for evidence in &packet.evidence {
        for reference in &evidence.references {
            let key = format!(
                "{}|{}|{}",
                reference.title.as_deref().unwrap_or_default(),
                reference.url.as_deref().unwrap_or_default(),
                reference.path.as_deref().unwrap_or_default()
            );
            if seen.insert(key) {
                references.push(reference.clone());
            }
        }
    }
    references
}

fn dedup_strings(values: Vec<String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut deduped = Vec::new();
    for value in values {
        if seen.insert(value.clone()) {
            deduped.push(value);
        }
    }
    deduped
}

fn redact_value(value: &mut Value, key: Option<&str>, masked_values: &mut usize) {
    match value {
        Value::Object(map) => {
            for (child_key, child_value) in map {
                redact_value(child_value, Some(child_key), masked_values);
            }
        }
        Value::Array(items) => {
            for item in items {
                redact_value(item, key, masked_values);
            }
        }
        Value::String(text) => {
            if (key.is_some_and(secret_key_name) || looks_token_like(text))
                && !text.is_empty()
                && text != "[REDACTED]"
            {
                *text = "[REDACTED]".to_string();
                *masked_values += 1;
            } else {
                let (redacted, masked) = redact_inline_secret_values(text);
                if masked > 0 {
                    *text = redacted;
                    *masked_values += masked;
                }
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

fn secret_key_name(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    [
        "api_key",
        "apikey",
        "authorization",
        "credential",
        "password",
        "passwd",
        "private_key",
        "secret",
        "token",
        "access_key",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn looks_token_like(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.starts_with("sk-") || trimmed.starts_with("ghp_") || trimmed.starts_with("xox") {
        return true;
    }
    if trimmed.len() >= 30
        && trimmed.matches('.').count() >= 2
        && !trimmed.contains(char::is_whitespace)
    {
        return true;
    }
    trimmed.len() >= 40
        && !trimmed.contains(char::is_whitespace)
        && trimmed.chars().any(|char| char.is_ascii_alphabetic())
        && trimmed.chars().any(|char| char.is_ascii_digit())
}

fn redact_inline_secret_values(input: &str) -> (String, usize) {
    let (with_assignments, assignment_count) = redact_secret_assignments(input);
    let (with_tokens, token_count) = redact_token_prefixes(&with_assignments);
    (with_tokens, assignment_count + token_count)
}

fn redact_secret_assignments(input: &str) -> (String, usize) {
    let patterns = [
        "api_token=",
        "api-key=",
        "api_key=",
        "apikey=",
        "authorization=",
        "password=",
        "secret=",
        "token=",
    ];
    let lower = input.to_ascii_lowercase();
    let mut output = String::with_capacity(input.len());
    let mut index = 0;
    let mut masked = 0;

    while index < input.len() {
        let Some((pattern_index, pattern)) = patterns
            .iter()
            .filter_map(|pattern| {
                lower[index..]
                    .find(pattern)
                    .map(|offset| (index + offset, *pattern))
            })
            .min_by_key(|(position, _)| *position)
        else {
            output.push_str(&input[index..]);
            break;
        };

        output.push_str(&input[index..pattern_index]);
        let value_start = pattern_index + pattern.len();
        output.push_str(&input[pattern_index..value_start]);

        let value_end = input[value_start..]
            .find(secret_value_terminator)
            .map(|offset| value_start + offset)
            .unwrap_or(input.len());

        if value_end > value_start {
            output.push_str("[REDACTED]");
            masked += 1;
        }
        index = value_end;
    }

    (output, masked)
}

fn secret_value_terminator(character: char) -> bool {
    character.is_whitespace() || matches!(character, ',' | ';' | '"' | '\'' | ')' | ']')
}

fn redact_token_prefixes(input: &str) -> (String, usize) {
    let prefixes = ["sk-", "ghp_", "xox", "cfut_"];
    let mut output = String::with_capacity(input.len());
    let mut index = 0;
    let mut masked = 0;

    while index < input.len() {
        let Some((prefix_index, prefix)) = prefixes
            .iter()
            .filter_map(|prefix| {
                input[index..]
                    .find(prefix)
                    .map(|offset| (index + offset, *prefix))
            })
            .min_by_key(|(position, _)| *position)
        else {
            output.push_str(&input[index..]);
            break;
        };

        output.push_str(&input[index..prefix_index]);
        let token_end = input[prefix_index..]
            .find(|character: char| !is_token_character(character))
            .map(|offset| prefix_index + offset)
            .unwrap_or(input.len());

        if token_end > prefix_index + prefix.len() {
            output.push_str("[REDACTED]");
            masked += 1;
        } else {
            output.push_str(prefix);
        }
        index = token_end;
    }

    (output, masked)
}

fn is_token_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
}

fn display_path(path: &Path) -> String {
    path.display().to_string()
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{BTreeMap, BTreeSet},
        fs,
        io::{Read, Write},
        net::TcpListener,
        thread,
    };

    use async_trait::async_trait;
    use tempfile::TempDir;

    use super::*;

    struct MockProvider;

    type AgentFixturePaths = (PathBuf, PathBuf, PathBuf, PathBuf, PathBuf);
    type RequestLogHandle = thread::JoinHandle<Vec<String>>;

    #[async_trait]
    impl LlmProvider for MockProvider {
        async fn reason(&self, _packet: &EvidencePacket) -> Result<ProviderResponse, LlmError> {
            Ok(ProviderResponse {
                reasoning: ReasoningResult {
                    summary: "Mock LLM summary.".to_string(),
                    hypotheses: vec![Hypothesis {
                        id: "hyp-1".to_string(),
                        title: "Mock hypothesis".to_string(),
                        description: "The mock provider returned a valid hypothesis.".to_string(),
                        confidence: 0.7,
                        supporting_evidence_ids: Vec::new(),
                        contradicting_evidence_ids: Vec::new(),
                        missing_checks: Vec::new(),
                        risk_if_wrong: "The mock could hide provider failures.".to_string(),
                    }],
                    missing_checks: Vec::new(),
                    recommended_checks: vec![RecommendedCheck {
                        id: "check-1".to_string(),
                        title: "Review service dashboard".to_string(),
                        description: "Inspect read-only dashboard panels for the alert window."
                            .to_string(),
                        target: Some("service:web".to_string()),
                        reason: "The check validates the impact scope.".to_string(),
                        read_only: true,
                        source: "mock".to_string(),
                        related_evidence_ids: Vec::new(),
                    }],
                    risk_notes: Vec::new(),
                    operator_notes: Vec::new(),
                    confidence_notes: Vec::new(),
                },
                metadata: LlmExchangeMetadata {
                    provider: "mock".to_string(),
                    model: "mock-model".to_string(),
                    request_id: Some("mock-request".to_string()),
                    response_metadata: BTreeMap::new(),
                },
                raw_response: json!({ "mock": true }),
            })
        }
    }

    fn write_example_inputs(
        dir: &Path,
    ) -> Result<(PathBuf, PathBuf, PathBuf), Box<dyn std::error::Error>> {
        let inventory_path = dir.join("inventory.yaml");
        let alert_path = dir.join("alert.yaml");
        let runbook_path = dir.join("runbook.yaml");

        fs::write(
            &inventory_path,
            r#"
targets:
  - id: service:web
    kind: service
    name: web
    environment: prod
    service: web
    criticality: high
"#,
        )?;
        fs::write(
            &alert_path,
            r#"
id: web-5xx
name: WebHigh5xx
severity: page
status: firing
summary: Web 5xx responses are above threshold.
target: service:web
started_at: "2026-06-29T00:00:00Z"
annotations:
  sample_api_key: placeholder-for-redaction-test
"#,
        )?;
        fs::write(
            &runbook_path,
            r#"
id: web-5xx
title: Web 5xx investigation
applies_to:
  - service:web
checks:
  - id: dashboard
    title: Review web error dashboard
    description: Compare error rate and latency charts around the alert window.
    read_only: true
"#,
        )?;

        Ok((inventory_path, alert_path, runbook_path))
    }

    #[test]
    fn parses_input_files() -> Result<(), Box<dyn std::error::Error>> {
        let temp = TempDir::new()?;
        let (inventory_path, alert_path, runbook_path) = write_example_inputs(temp.path())?;

        let inventory = load_inventory(&inventory_path)?;
        let alert = load_alert(&alert_path)?;
        let runbook = load_runbook(&runbook_path)?;

        assert_eq!(inventory.targets.len(), 1);
        assert_eq!(alert.id, "web-5xx");
        assert_eq!(runbook.checks.len(), 1);
        Ok(())
    }

    #[test]
    fn redacts_secret_like_values() -> Result<(), Box<dyn std::error::Error>> {
        let mut packet = EvidencePacket {
            investigation_id: "test".to_string(),
            question: "test".to_string(),
            targets: Vec::new(),
            alerts: Vec::new(),
            evidence: Vec::new(),
            runbooks: Vec::new(),
            constraints: InvestigationConstraints::default(),
            redaction: RedactionReport::default(),
            metadata: BTreeMap::from([(
                "api_token".to_string(),
                Value::String("placeholder-for-redaction-test".to_string()),
            )]),
        };
        packet = redact_evidence_packet(packet)?;

        assert_eq!(packet.redaction.masked_values, 1);
        assert_eq!(packet.metadata["api_token"], "[REDACTED]");
        Ok(())
    }

    #[test]
    fn redacts_inline_secret_values_in_log_content() -> Result<(), Box<dyn std::error::Error>> {
        let mut packet = EvidencePacket {
            investigation_id: "test".to_string(),
            question: "test".to_string(),
            targets: Vec::new(),
            alerts: Vec::new(),
            evidence: vec![Evidence {
                id: "log-001".to_string(),
                kind: EvidenceKind::Log,
                summary: "log with inline secret".to_string(),
                source: EvidenceSource {
                    kind: "test".to_string(),
                    name: "test".to_string(),
                    path: None,
                },
                target: None,
                timestamp: None,
                confidence: 1.0,
                data: json!({
                    "content": "auth failed api_token=sk-redaction-placeholder-1234567890"
                }),
                references: Vec::new(),
            }],
            runbooks: Vec::new(),
            constraints: InvestigationConstraints::default(),
            redaction: RedactionReport::default(),
            metadata: BTreeMap::new(),
        };

        packet = redact_evidence_packet(packet)?;
        let content = packet.evidence[0].data["content"]
            .as_str()
            .ok_or("missing content")?;
        assert!(content.contains("api_token=[REDACTED]"));
        assert!(!content.contains("sk-redaction-placeholder-1234567890"));
        Ok(())
    }

    #[tokio::test]
    async fn investigates_with_no_llm() -> Result<(), Box<dyn std::error::Error>> {
        let temp = TempDir::new()?;
        let (inventory_path, alert_path, runbook_path) = write_example_inputs(temp.path())?;

        let outcome = investigate(
            InvestigationRequest {
                alert_path: Some(alert_path),
                inventory_path,
                runbook_paths: vec![runbook_path],
                runbook_dir: None,
                target: None,
                no_llm: true,
                dry_run: false,
            },
            None,
        )
        .await?;

        assert!(outcome.brief.summary.contains("Deterministic"));
        assert!(outcome.trajectory.llm.is_none());
        assert!(outcome.trajectory.evidence_packet.redaction.masked_values > 0);
        Ok(())
    }

    #[tokio::test]
    async fn investigates_with_mock_provider() -> Result<(), Box<dyn std::error::Error>> {
        let temp = TempDir::new()?;
        let (inventory_path, alert_path, runbook_path) = write_example_inputs(temp.path())?;
        let provider = MockProvider;

        let outcome = investigate(
            InvestigationRequest {
                alert_path: Some(alert_path),
                inventory_path,
                runbook_paths: vec![runbook_path],
                runbook_dir: None,
                target: None,
                no_llm: false,
                dry_run: false,
            },
            Some(&provider),
        )
        .await?;

        assert_eq!(outcome.brief.summary, "Mock LLM summary.");
        assert_eq!(
            outcome
                .trajectory
                .llm
                .as_ref()
                .map(|metadata| metadata.provider.as_str()),
            Some("mock")
        );
        Ok(())
    }

    fn write_agent_fixtures(dir: &Path) -> Result<AgentFixturePaths, Box<dyn std::error::Error>> {
        let (inventory_path, _alert_path, runbook_path) = write_example_inputs(dir)?;
        let alertmanager_path = dir.join("alertmanager.yaml");
        let prometheus_path = dir.join("prometheus.yaml");
        let github_path = dir.join("github.yaml");

        fs::write(
            &alertmanager_path,
            r#"
alerts:
  - id: web-5xx-live
    name: WebHigh5xx
    severity: page
    status: firing
    summary: Web 5xx responses are still firing in Alertmanager.
    target: service:web
    started_at: "2026-06-29T00:10:00Z"
    labels:
      service: web
"#,
        )?;
        fs::write(
            &prometheus_path,
            r#"
status: success
data:
  resultType: vector
  result:
    - metric:
        service: web
      value:
        - 1780000000
        - "8.4"
"#,
        )?;
        fs::write(
            &github_path,
            r#"
changes:
  - title: Adjust upstream timeout
    url: https://github.com/example/web/pull/123
    merged_at: "2026-06-29T00:00:00Z"
"#,
        )?;

        Ok((
            inventory_path,
            runbook_path,
            alertmanager_path,
            prometheus_path,
            github_path,
        ))
    }

    fn agent_sources(
        inventory_path: PathBuf,
        runbook_path: PathBuf,
        alertmanager_path: PathBuf,
        prometheus_path: PathBuf,
        github_path: PathBuf,
    ) -> Vec<SourceConfig> {
        vec![
            SourceConfig::InventoryFile {
                name: "local".to_string(),
                path: Some(inventory_path),
            },
            SourceConfig::RunbookFile {
                name: "local".to_string(),
                dir: None,
                paths: vec![runbook_path],
            },
            SourceConfig::Alertmanager {
                name: "prod".to_string(),
                url: None,
                fixture_path: Some(alertmanager_path),
                bearer_token_env: None,
            },
            SourceConfig::Prometheus {
                name: "prod".to_string(),
                url: None,
                fixture_path: Some(prometheus_path),
                bearer_token_env: None,
            },
            SourceConfig::Github {
                name: "main".to_string(),
                api_url: None,
                repo: Some("example/web".to_string()),
                fixture_path: Some(github_path),
                bearer_token_env: None,
            },
        ]
    }

    fn start_json_server(
        body: String,
        expected_requests: usize,
    ) -> Result<(String, RequestLogHandle), Box<dyn std::error::Error>> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let address = listener.local_addr()?;
        let handle = thread::spawn(move || {
            let mut requests = Vec::new();
            for _ in 0..expected_requests {
                let Ok((mut stream, _)) = listener.accept() else {
                    break;
                };
                let mut buffer = [0; 16_384];
                let bytes_read = stream.read(&mut buffer).unwrap_or(0);
                requests.push(String::from_utf8_lossy(&buffer[..bytes_read]).to_string());
                let response = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.write_all(response.as_bytes());
            }
            requests
        });
        Ok((format!("http://{address}"), handle))
    }

    #[tokio::test]
    async fn plan_only_agent_investigation_lists_read_only_calls(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let temp = TempDir::new()?;
        let (inventory_path, runbook_path, alertmanager_path, prometheus_path, github_path) =
            write_agent_fixtures(temp.path())?;

        let outcome = plan_agent_investigation(
            AgentInvestigationRequest {
                selector: InvestigationSelector::Target("service:web".to_string()),
                since: Some("30m".to_string()),
                sources: agent_sources(
                    inventory_path,
                    runbook_path,
                    alertmanager_path,
                    prometheus_path,
                    github_path,
                ),
                source_filters: Vec::new(),
                budget: InvestigationBudget {
                    max_iterations: 1,
                    max_tool_calls: 8,
                    max_duration_secs: 60,
                },
                no_llm: true,
                dry_run: false,
                plan_only: true,
            },
            None,
        )
        .await?;

        let capability_ids = outcome
            .plan
            .calls
            .iter()
            .map(|call| call.capability_id.as_str())
            .collect::<BTreeSet<_>>();
        assert!(capability_ids.contains("inventory_lookup"));
        assert!(capability_ids.contains("runbook_lookup"));
        assert!(capability_ids.contains("alertmanager_active_alerts"));
        assert!(capability_ids.contains("prometheus_query"));
        assert!(capability_ids.contains("github_recent_changes"));
        assert!(outcome
            .capabilities
            .iter()
            .all(|capability| capability.read_only));
        Ok(())
    }

    #[tokio::test]
    async fn agent_investigation_collects_fixture_evidence(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let temp = TempDir::new()?;
        let (inventory_path, runbook_path, alertmanager_path, prometheus_path, github_path) =
            write_agent_fixtures(temp.path())?;

        let outcome = investigate_agent(
            AgentInvestigationRequest {
                selector: InvestigationSelector::Target("service:web".to_string()),
                since: Some("30m".to_string()),
                sources: agent_sources(
                    inventory_path,
                    runbook_path,
                    alertmanager_path,
                    prometheus_path,
                    github_path,
                ),
                source_filters: Vec::new(),
                budget: InvestigationBudget {
                    max_iterations: 1,
                    max_tool_calls: 8,
                    max_duration_secs: 60,
                },
                no_llm: true,
                dry_run: false,
                plan_only: false,
            },
            None,
        )
        .await?;

        let loop_record = outcome
            .trajectory
            .investigation_loop
            .as_ref()
            .ok_or("missing investigation loop")?;
        assert_eq!(loop_record.iterations.len(), 1);
        assert!(loop_record.iterations[0]
            .results
            .iter()
            .any(|result| result.status == ToolResultStatus::Succeeded));
        assert!(outcome
            .trajectory
            .evidence_packet
            .evidence
            .iter()
            .any(|item| item.kind == EvidenceKind::Metric));
        assert!(outcome
            .trajectory
            .evidence_packet
            .evidence
            .iter()
            .any(|item| item.kind == EvidenceKind::Change));
        assert!(outcome
            .trajectory
            .evidence_packet
            .evidence
            .iter()
            .any(|item| item.source.kind == "alertmanager"));
        Ok(())
    }

    #[tokio::test]
    async fn agent_investigation_collects_live_required_adapter_evidence(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let alertmanager_body = serde_json::to_string(&json!([{
            "fingerprint": "alert-fingerprint",
            "labels": {
                "alertname": "WebHigh5xxRate",
                "service": "web",
                "severity": "page"
            },
            "annotations": {
                "summary": "Web 5xx responses are firing."
            },
            "startsAt": "2026-06-29T00:10:00Z",
            "status": {
                "state": "active"
            }
        }]))?;
        let prometheus_body = serde_json::to_string(&json!({
            "status": "success",
            "data": {
                "resultType": "matrix",
                "result": []
            }
        }))?;
        let github_body = serde_json::to_string(&json!([{
            "sha": "abc123",
            "html_url": "https://github.com/example/web/commit/abc123",
            "commit": {
                "message": "Adjust web timeout",
                "author": {
                    "date": "2026-06-29T00:00:00Z"
                }
            }
        }]))?;
        let (alertmanager_url, alertmanager_handle) = start_json_server(alertmanager_body, 1)?;
        let (prometheus_url, prometheus_handle) = start_json_server(prometheus_body, 2)?;
        let (github_url, github_handle) = start_json_server(github_body, 1)?;

        let outcome = investigate_agent(
            AgentInvestigationRequest {
                selector: InvestigationSelector::Target("service:web".to_string()),
                since: Some("30m".to_string()),
                sources: vec![
                    SourceConfig::Alertmanager {
                        name: "prod".to_string(),
                        url: Some(alertmanager_url),
                        fixture_path: None,
                        bearer_token_env: None,
                    },
                    SourceConfig::Prometheus {
                        name: "prod".to_string(),
                        url: Some(prometheus_url),
                        fixture_path: None,
                        bearer_token_env: None,
                    },
                    SourceConfig::Github {
                        name: "main".to_string(),
                        api_url: Some(github_url),
                        repo: Some("example/web".to_string()),
                        fixture_path: None,
                        bearer_token_env: None,
                    },
                ],
                source_filters: Vec::new(),
                budget: InvestigationBudget {
                    max_iterations: 1,
                    max_tool_calls: 8,
                    max_duration_secs: 60,
                },
                no_llm: true,
                dry_run: false,
                plan_only: false,
            },
            None,
        )
        .await?;

        let evidence = &outcome.trajectory.evidence_packet.evidence;
        assert!(evidence
            .iter()
            .any(|item| item.source.kind == "alertmanager"));
        assert!(evidence
            .iter()
            .any(|item| item.kind == EvidenceKind::Metric));
        assert!(evidence
            .iter()
            .any(|item| item.kind == EvidenceKind::Change));
        let alertmanager_requests = alertmanager_handle
            .join()
            .map_err(|_| "alertmanager server thread panicked")?;
        let prometheus_requests = prometheus_handle
            .join()
            .map_err(|_| "prometheus server thread panicked")?;
        let github_requests = github_handle
            .join()
            .map_err(|_| "github server thread panicked")?;
        assert_eq!(alertmanager_requests.len(), 1);
        assert_eq!(prometheus_requests.len(), 2);
        assert_eq!(github_requests.len(), 1);
        assert!(github_requests[0].contains("/repos/example/web/commits"));
        Ok(())
    }

    #[tokio::test]
    async fn agent_investigation_collects_optional_adapter_evidence(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let temp = TempDir::new()?;
        let dns_path = temp.path().join("dns.yaml");
        let loki_path = temp.path().join("loki.yaml");
        let grafana_path = temp.path().join("grafana.yaml");
        let kubernetes_path = temp.path().join("kubernetes.yaml");
        fs::write(
            &dns_path,
            r#"
host: web.example.com
addresses:
  - 203.0.113.10
"#,
        )?;
        fs::write(
            &loki_path,
            r#"
status: success
data:
  result:
    - stream:
        service: web
      values: []
"#,
        )?;
        fs::write(
            &grafana_path,
            r#"
- id: 1
  text: deploy web
  tags:
    - web
"#,
        )?;
        fs::write(
            &kubernetes_path,
            r#"
items:
  - metadata:
      name: web-event
    reason: Pulled
"#,
        )?;
        let http_body = serde_json::to_string(&json!({"ok": true}))?;
        let (http_url, http_handle) = start_json_server(http_body, 1)?;

        let outcome = investigate_agent(
            AgentInvestigationRequest {
                selector: InvestigationSelector::Target("service:web".to_string()),
                since: Some("30m".to_string()),
                sources: vec![
                    SourceConfig::Http {
                        name: "web".to_string(),
                        url: Some(http_url),
                        fixture_path: None,
                        bearer_token_env: None,
                    },
                    SourceConfig::Dns {
                        name: "web".to_string(),
                        fixture_path: Some(dns_path),
                    },
                    SourceConfig::Loki {
                        name: "prod".to_string(),
                        url: None,
                        fixture_path: Some(loki_path),
                        bearer_token_env: None,
                    },
                    SourceConfig::Grafana {
                        name: "prod".to_string(),
                        url: None,
                        fixture_path: Some(grafana_path),
                        bearer_token_env: None,
                    },
                    SourceConfig::Kubernetes {
                        name: "prod".to_string(),
                        url: None,
                        namespace: Some("default".to_string()),
                        fixture_path: Some(kubernetes_path),
                        bearer_token_env: None,
                    },
                ],
                source_filters: Vec::new(),
                budget: InvestigationBudget {
                    max_iterations: 1,
                    max_tool_calls: 8,
                    max_duration_secs: 60,
                },
                no_llm: true,
                dry_run: false,
                plan_only: false,
            },
            None,
        )
        .await?;

        let evidence = &outcome.trajectory.evidence_packet.evidence;
        assert!(evidence.iter().any(|item| item.source.kind == "http"));
        assert!(evidence.iter().any(|item| item.source.kind == "dns"));
        assert!(evidence.iter().any(|item| item.source.kind == "loki"));
        assert!(evidence.iter().any(|item| item.source.kind == "grafana"));
        assert!(evidence.iter().any(|item| item.source.kind == "kubernetes"));
        let http_requests = http_handle
            .join()
            .map_err(|_| "http server thread panicked")?;
        assert_eq!(http_requests.len(), 1);
        Ok(())
    }

    #[test]
    fn case_init_refuses_existing_without_force() -> Result<(), Box<dyn std::error::Error>> {
        let temp = TempDir::new()?;
        let case_dir = temp.path().join("web-5xx");
        let request = CaseInitRequest {
            case_dir: case_dir.clone(),
            target: "service:web".to_string(),
            severity: "page".to_string(),
            summary: "Web 5xx responses are above threshold.".to_string(),
            force: false,
        };

        let manifest = init_case(request.clone())?;
        assert_eq!(manifest.id, "web-5xx");
        assert!(case_dir.join("vigil.yaml").is_file());
        assert!(case_dir.join("evidence").is_dir());
        assert!(case_dir.join("runbooks").is_dir());
        assert!(case_dir.join("output").is_dir());

        let error = init_case(request).err();
        assert!(matches!(error, Some(CoreError::CaseAlreadyExists { .. })));

        let overwritten = init_case(CaseInitRequest {
            force: true,
            case_dir,
            target: "service:web".to_string(),
            severity: "page".to_string(),
            summary: "Updated summary.".to_string(),
        })?;
        assert_eq!(overwritten.summary, "Updated summary.");
        Ok(())
    }

    #[tokio::test]
    async fn case_workflow_intakes_evidence_and_investigates(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let temp = TempDir::new()?;
        let (_inventory_path, _alert_path, runbook_path) = write_example_inputs(temp.path())?;
        let case_dir = temp.path().join("web-5xx");

        init_case(CaseInitRequest {
            case_dir: case_dir.clone(),
            target: "service:web".to_string(),
            severity: "page".to_string(),
            summary: "Web service 5xx responses are above threshold.".to_string(),
            force: false,
        })?;

        let metric = add_case_evidence(EvidenceAddRequest {
            case_dir: case_dir.clone(),
            kind: EvidenceKind::Metric,
            summary: "HTTP 5xx rate increased from 0.2% to 8.4%.".to_string(),
            source: "prometheus".to_string(),
            url: Some("https://grafana.example.com/d/web".to_string()),
            file: None,
        })?;
        assert_eq!(
            metric.path.file_name().and_then(|name| name.to_str()),
            Some("metric-001.yaml")
        );
        assert_eq!(load_case_evidence(&metric.path)?.kind, EvidenceKind::Metric);

        let change = add_case_change(ChangeAddRequest {
            case_dir: case_dir.clone(),
            summary: "Caddy upstream timeout setting changed before the alert.".to_string(),
            source: "github".to_string(),
            url: Some("https://github.com/example/repo/pull/123".to_string()),
        })?;
        assert_eq!(load_case_evidence(&change.path)?.kind, EvidenceKind::Change);

        let copied_runbook = add_case_runbook(RunbookAddRequest {
            case_dir: case_dir.clone(),
            runbook_path,
        })?;
        assert!(copied_runbook.is_file());

        let outcome = investigate_case(
            CaseInvestigationRequest {
                case_dir: case_dir.clone(),
                no_llm: true,
                dry_run: false,
            },
            None,
        )
        .await?;

        assert!(outcome.brief.summary.contains("Deterministic"));
        let expected_case_dir = case_dir.display().to_string();
        assert_eq!(
            outcome.trajectory.inputs.case_dir.as_deref(),
            Some(expected_case_dir.as_str())
        );
        assert!(outcome
            .trajectory
            .evidence_packet
            .evidence
            .iter()
            .any(|item| item.kind == EvidenceKind::Change));
        assert!(outcome.trajectory.evidence_packet.runbooks.len() == 1);
        Ok(())
    }
}
