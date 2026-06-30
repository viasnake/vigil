use serde::Serialize;
use thiserror::Error;
use vigil_model::{EvidenceBrief, SourceReference, ToolPlan, Trajectory};

#[derive(Debug, Error)]
pub enum RenderError {
    #[error("JSON output could not be rendered: {0}")]
    Json(#[from] serde_json::Error),
}

pub fn render_markdown(brief: &EvidenceBrief) -> String {
    let mut out = String::new();
    push_heading(&mut out, 1, &brief.title);
    push_paragraph(&mut out, &brief.summary);

    push_heading(&mut out, 2, "Affected Targets");
    if brief.targets.is_empty() {
        push_bullet(
            &mut out,
            "No target was resolved from the supplied evidence.",
        );
    } else {
        for target in &brief.targets {
            let mut parts = vec![
                format!("`{}`", target.id),
                format!("{:?}", target.kind).to_ascii_lowercase(),
                target.name.clone(),
            ];
            if let Some(environment) = &target.environment {
                parts.push(format!("env: {environment}"));
            }
            if let Some(criticality) = &target.criticality {
                parts.push(format!("criticality: {criticality}"));
            }
            push_bullet(&mut out, &parts.join(" - "));
        }
    }

    push_heading(&mut out, 2, "Observed Evidence");
    if brief.evidence.is_empty() {
        push_bullet(&mut out, "No observed evidence was supplied.");
    } else {
        for evidence in &brief.evidence {
            let mut text = format!(
                "`{}` - {:?}: {} (source: {})",
                evidence.id,
                evidence.kind,
                evidence.summary,
                source_label(&evidence.source)
            );
            if let Some(target) = &evidence.target {
                text.push_str(&format!(", target: `{target}`"));
            }
            push_bullet(&mut out, &text);
        }
    }

    push_heading(&mut out, 2, "Hypotheses");
    if brief.hypotheses.is_empty() {
        push_bullet(&mut out, "No hypotheses were produced.");
    } else {
        for hypothesis in &brief.hypotheses {
            push_bullet(
                &mut out,
                &format!(
                    "`{}` - {} (confidence {:.2}): {}",
                    hypothesis.id, hypothesis.title, hypothesis.confidence, hypothesis.description
                ),
            );
            if !hypothesis.supporting_evidence_ids.is_empty() {
                push_indented_bullet(
                    &mut out,
                    &format!(
                        "Supporting evidence: {}",
                        hypothesis.supporting_evidence_ids.join(", ")
                    ),
                );
            }
            if !hypothesis.contradicting_evidence_ids.is_empty() {
                push_indented_bullet(
                    &mut out,
                    &format!(
                        "Contradicting evidence: {}",
                        hypothesis.contradicting_evidence_ids.join(", ")
                    ),
                );
            }
            push_indented_bullet(
                &mut out,
                &format!("Risk if wrong: {}", hypothesis.risk_if_wrong),
            );
        }
    }

    push_heading(&mut out, 2, "Missing Checks");
    if brief.missing_checks.is_empty() {
        push_bullet(&mut out, "No missing checks were identified.");
    } else {
        for check in &brief.missing_checks {
            let target = check
                .target
                .as_ref()
                .map(|target| format!(" target: `{target}`;"))
                .unwrap_or_default();
            push_bullet(
                &mut out,
                &format!(
                    "`{}` - {}:{} {} Reason: {}",
                    check.id, check.title, target, check.description, check.reason
                ),
            );
        }
    }

    push_heading(&mut out, 2, "Recommended Read-Only Checks");
    if brief.recommended_checks.is_empty() {
        push_bullet(&mut out, "No recommended checks were produced.");
    } else {
        for check in &brief.recommended_checks {
            let target = check
                .target
                .as_ref()
                .map(|target| format!(" target: `{target}`;"))
                .unwrap_or_default();
            push_bullet(
                &mut out,
                &format!(
                    "`{}` - {}:{} {} Reason: {}. Source: {}. Not executed by Vigil.",
                    check.id, check.title, target, check.description, check.reason, check.source
                ),
            );
        }
    }

    if !brief.risk_notes.is_empty() {
        push_heading(&mut out, 2, "Risk Notes");
        for note in &brief.risk_notes {
            push_bullet(&mut out, note);
        }
    }

    if !brief.references.is_empty() {
        push_heading(&mut out, 2, "Source References");
        for reference in &brief.references {
            push_bullet(&mut out, &reference_label(reference));
        }
    }

    if !brief.warnings.is_empty() {
        push_heading(&mut out, 2, "Warnings");
        for warning in &brief.warnings {
            push_bullet(&mut out, warning);
        }
    }

    out
}

pub fn render_json<T: Serialize>(value: &T) -> Result<String, RenderError> {
    serde_json::to_string_pretty(value).map_err(RenderError::Json)
}

pub fn render_trajectory_json(trajectory: &Trajectory) -> Result<String, RenderError> {
    render_json(trajectory)
}

pub fn render_tool_plan(plan: &ToolPlan) -> String {
    let mut out = String::new();
    push_heading(&mut out, 1, "Planned Read-Only Collection");
    if plan.rationale.trim().is_empty() {
        push_paragraph(&mut out, "No planning rationale was provided.");
    } else {
        push_paragraph(&mut out, &plan.rationale);
    }

    if plan.calls.is_empty() {
        push_bullet(&mut out, "No read-only tool calls were proposed.");
    } else {
        for call in &plan.calls {
            let mut parts = vec![
                format!("`{}`", call.capability_id),
                format!("source: `{}`", call.source_id),
            ];
            if let Some(target) = &call.target {
                parts.push(format!("target: `{target}`"));
            }
            if let Some(since) = &call.since {
                parts.push(format!("since: `{since}`"));
            }
            parts.push(format!("reason: {}", call.reason));
            push_bullet(&mut out, &parts.join(" - "));
        }
    }

    out
}

fn push_heading(out: &mut String, level: usize, text: &str) {
    out.push_str(&"#".repeat(level));
    out.push(' ');
    out.push_str(text);
    out.push_str("\n\n");
}

fn push_paragraph(out: &mut String, text: &str) {
    out.push_str(text.trim());
    out.push_str("\n\n");
}

fn push_bullet(out: &mut String, text: &str) {
    out.push_str("- ");
    out.push_str(text.trim());
    out.push('\n');
}

fn push_indented_bullet(out: &mut String, text: &str) {
    out.push_str("  - ");
    out.push_str(text.trim());
    out.push('\n');
}

fn source_label(source: &vigil_model::EvidenceSource) -> String {
    match &source.path {
        Some(path) => format!("{}:{} ({path})", source.kind, source.name),
        None => format!("{}:{}", source.kind, source.name),
    }
}

fn reference_label(reference: &SourceReference) -> String {
    match (&reference.title, &reference.url, &reference.path) {
        (Some(title), Some(url), _) => format!("{title} - {url}"),
        (Some(title), None, Some(path)) => format!("{title} - {path}"),
        (Some(title), None, None) => title.clone(),
        (None, Some(url), _) => url.clone(),
        (None, None, Some(path)) => path.clone(),
        (None, None, None) => "Unnamed reference".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use vigil_model::{
        Evidence, EvidenceBrief, EvidenceKind, EvidenceSource, Hypothesis, RecommendedCheck,
        Target, TargetKind,
    };

    use super::*;

    fn brief() -> EvidenceBrief {
        EvidenceBrief {
            title: "Investigation Brief: web".to_string(),
            summary: "Elevated 5xx responses are affecting the web service.".to_string(),
            targets: vec![Target {
                id: "service:web".to_string(),
                kind: TargetKind::Service,
                name: "web".to_string(),
                environment: Some("prod".to_string()),
                service: Some("web".to_string()),
                host: None,
                labels: BTreeMap::new(),
                criticality: Some("high".to_string()),
                metadata: BTreeMap::new(),
            }],
            evidence: vec![Evidence {
                id: "alert:web-5xx".to_string(),
                kind: EvidenceKind::Alert,
                summary: "5xx rate is above threshold".to_string(),
                source: EvidenceSource {
                    kind: "file".to_string(),
                    name: "alert".to_string(),
                    path: Some("alert.yaml".to_string()),
                },
                target: Some("service:web".to_string()),
                timestamp: None,
                confidence: 1.0,
                data: serde_json::json!({}),
                references: Vec::new(),
            }],
            hypotheses: vec![Hypothesis {
                id: "hyp-1".to_string(),
                title: "Dependency errors".to_string(),
                description: "A downstream dependency may be failing.".to_string(),
                confidence: 0.5,
                supporting_evidence_ids: vec!["alert:web-5xx".to_string()],
                contradicting_evidence_ids: Vec::new(),
                missing_checks: Vec::new(),
                risk_if_wrong: "The team may inspect the wrong dependency.".to_string(),
            }],
            missing_checks: Vec::new(),
            recommended_checks: vec![RecommendedCheck {
                id: "check-1".to_string(),
                title: "Review dependency dashboard".to_string(),
                description: "Compare dependency errors with the alert window.".to_string(),
                target: Some("service:web".to_string()),
                reason: "The check is read-only and validates the hypothesis.".to_string(),
                read_only: true,
                source: "test".to_string(),
                related_evidence_ids: Vec::new(),
            }],
            risk_notes: vec!["Hypotheses are not facts.".to_string()],
            references: Vec::new(),
            warnings: vec!["Recommended checks were not executed.".to_string()],
        }
    }

    #[test]
    fn renders_readable_markdown() {
        let markdown = render_markdown(&brief());
        assert_eq!(
            markdown,
            include_str!("../tests/fixtures/evidence_brief.md")
        );
    }

    #[test]
    fn renders_machine_readable_json() -> Result<(), Box<dyn std::error::Error>> {
        let json = render_json(&brief())?;
        let value: serde_json::Value = serde_json::from_str(&json)?;
        assert_eq!(value["title"], "Investigation Brief: web");
        Ok(())
    }
}
