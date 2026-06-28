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
    validate_reasoning_result, Alert, Evidence, EvidenceBrief, EvidenceKind, EvidencePacket,
    EvidenceSource, Hypothesis, Inventory, InvestigationConstraints, LlmExchangeMetadata,
    MissingCheck, ReasoningResult, RecommendedCheck, RedactionReport, Runbook, SourceReference,
    Target, TargetKind, Trajectory, TrajectoryInputs,
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
    #[error("runbook directory '{path}' is not a directory")]
    RunbookDirNotDirectory { path: String },
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
}
