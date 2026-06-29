use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use chrono::Utc;
use serde_json::{json, Value};
use thiserror::Error;
use uuid::Uuid;
use vigil_llm::{LlmError, LlmProvider, ProviderResponse};
use vigil_model::{
    validate_reasoning_result, Alert, CaseManifest, Evidence, EvidenceBrief, EvidenceKind,
    EvidencePacket, EvidenceSource, Hypothesis, Inventory, InvestigationConstraints,
    LlmExchangeMetadata, MissingCheck, ReasoningResult, RecommendedCheck, RedactionReport, Runbook,
    SourceReference, Target, TargetKind, Trajectory, TrajectoryInputs,
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
    use std::{collections::BTreeMap, fs};

    use async_trait::async_trait;
    use tempfile::TempDir;

    use super::*;

    struct MockProvider;

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
