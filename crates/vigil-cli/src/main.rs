use std::{
    fs,
    path::{Path, PathBuf},
    process,
    str::FromStr,
};

use anyhow::{anyhow, Context, Result};
use clap::{Args, Parser, Subcommand};
use vigil_config::{
    check_output_path, check_readable_file, load_config, CloudflareEndpoint, ConfigOverrides,
    OutputFormat, ResolvedConfig,
};
use vigil_core::{
    add_case_change, add_case_evidence, add_case_runbook, init_case, investigate,
    investigate_agent, investigate_case, load_trajectory, plan_agent_investigation,
    validate_input_files, AgentInvestigationRequest, CaseInitRequest, CaseInvestigationRequest,
    ChangeAddRequest, EvidenceAddRequest, InvestigationRequest, InvestigationSelector,
    RunbookAddRequest, SourceConfig, ValidationRequest,
};
use vigil_llm::{
    CloudflareAiGatewayConfig, CloudflareAiGatewayProvider, CloudflareEndpointStyle, LlmProvider,
};
use vigil_model::{EvidenceKind, InvestigationBudget};
use vigil_render::{render_json, render_markdown, render_tool_plan, render_trajectory_json};

#[derive(Debug, Parser)]
#[command(name = "vigil")]
#[command(about = "Generate evidence-backed SRE investigation briefs")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Case {
        #[command(subcommand)]
        command: CaseCommand,
    },
    Evidence {
        #[command(subcommand)]
        command: EvidenceCommand,
    },
    Change {
        #[command(subcommand)]
        command: ChangeCommand,
    },
    Runbook {
        #[command(subcommand)]
        command: RunbookCommand,
    },
    Investigate(Box<InvestigateArgs>),
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
    Validate(ValidateArgs),
    Render(RenderArgs),
    Version,
}

#[derive(Debug, Subcommand)]
enum CaseCommand {
    Init(CaseInitArgs),
}

#[derive(Debug, Args)]
struct CaseInitArgs {
    #[arg(value_name = "CASE_DIR")]
    case_dir: PathBuf,
    #[arg(long, value_name = "TARGET")]
    target: String,
    #[arg(long, value_name = "SEVERITY")]
    severity: String,
    #[arg(long, value_name = "SUMMARY")]
    summary: String,
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Subcommand)]
enum EvidenceCommand {
    Add(EvidenceAddArgs),
}

#[derive(Debug, Args)]
struct EvidenceAddArgs {
    #[arg(value_name = "CASE_DIR")]
    case_dir: PathBuf,
    #[arg(long, value_name = "KIND")]
    kind: String,
    #[arg(long, value_name = "SUMMARY")]
    summary: String,
    #[arg(long, value_name = "SOURCE")]
    source: String,
    #[arg(long, value_name = "URL")]
    url: Option<String>,
    #[arg(long, value_name = "PATH")]
    file: Option<PathBuf>,
}

#[derive(Debug, Subcommand)]
enum ChangeCommand {
    Add(ChangeAddArgs),
}

#[derive(Debug, Args)]
struct ChangeAddArgs {
    #[arg(value_name = "CASE_DIR")]
    case_dir: PathBuf,
    #[arg(long, value_name = "SUMMARY")]
    summary: String,
    #[arg(long, value_name = "SOURCE")]
    source: String,
    #[arg(long, value_name = "URL")]
    url: Option<String>,
}

#[derive(Debug, Subcommand)]
enum RunbookCommand {
    Add(RunbookAddArgs),
}

#[derive(Debug, Args)]
struct RunbookAddArgs {
    #[arg(value_name = "CASE_DIR")]
    case_dir: PathBuf,
    #[arg(value_name = "RUNBOOK")]
    runbook: PathBuf,
}

#[derive(Debug, Args)]
struct InvestigateArgs {
    #[arg(long, value_name = "PATH")]
    alert: Option<PathBuf>,
    #[arg(long, value_name = "PATH")]
    inventory: Option<PathBuf>,
    #[arg(long, value_name = "PATH")]
    runbook: Vec<PathBuf>,
    #[arg(long, value_name = "PATH")]
    runbook_dir: Option<PathBuf>,
    #[arg(long, value_name = "PATH")]
    output: Option<PathBuf>,
    #[arg(long, value_name = "PATH")]
    json_output: Option<PathBuf>,
    #[arg(long, value_name = "PATH")]
    trajectory_output: Option<PathBuf>,
    #[arg(long, value_name = "PATH")]
    config: Option<PathBuf>,
    #[arg(long, value_name = "MODEL")]
    model: Option<String>,
    #[arg(long, value_name = "ENDPOINT")]
    endpoint: Option<String>,
    #[arg(long, value_name = "GATEWAY_ID")]
    gateway_id: Option<String>,
    #[arg(long, value_name = "ACCOUNT_ID")]
    account_id: Option<String>,
    #[arg(long, value_name = "TOKEN")]
    api_token: Option<String>,
    #[arg(long, value_name = "SECONDS")]
    request_timeout_secs: Option<u64>,
    #[arg(long, value_name = "COUNT")]
    retry_count: Option<u32>,
    #[arg(long)]
    dry_run: bool,
    #[arg(long)]
    no_llm: bool,
    #[arg(long, value_name = "DURATION")]
    since: Option<String>,
    #[arg(long)]
    plan_only: bool,
    #[arg(long, value_name = "SOURCE")]
    source: Vec<String>,
    #[arg(long, value_name = "COUNT")]
    max_iterations: Option<u32>,
    #[arg(long, value_name = "COUNT")]
    max_tool_calls: Option<u32>,
    #[arg(long, value_name = "SECONDS")]
    max_duration_secs: Option<u64>,
    #[arg(value_name = "TARGET_OR_CASE", num_args = 0..=2)]
    target: Vec<String>,
}

#[derive(Debug, Subcommand)]
enum ConfigCommand {
    Check(ConfigCheckArgs),
}

#[derive(Debug, Args)]
struct ConfigCheckArgs {
    #[arg(long, value_name = "PATH")]
    config: Option<PathBuf>,
    #[arg(long, value_name = "PATH")]
    alert: Option<PathBuf>,
    #[arg(long, value_name = "PATH")]
    inventory: Option<PathBuf>,
    #[arg(long, value_name = "PATH")]
    output: Option<PathBuf>,
    #[arg(long, value_name = "MODEL")]
    model: Option<String>,
    #[arg(long, value_name = "ENDPOINT")]
    endpoint: Option<String>,
    #[arg(long, value_name = "GATEWAY_ID")]
    gateway_id: Option<String>,
    #[arg(long, value_name = "ACCOUNT_ID")]
    account_id: Option<String>,
    #[arg(long, value_name = "TOKEN")]
    api_token: Option<String>,
    #[arg(long, value_name = "SECONDS")]
    request_timeout_secs: Option<u64>,
    #[arg(long, value_name = "COUNT")]
    retry_count: Option<u32>,
}

#[derive(Debug, Args)]
struct ValidateArgs {
    #[arg(long, value_name = "PATH")]
    alert: Option<PathBuf>,
    #[arg(long, value_name = "PATH")]
    inventory: Option<PathBuf>,
    #[arg(long, value_name = "PATH")]
    runbook: Vec<PathBuf>,
    #[arg(long, value_name = "PATH")]
    runbook_dir: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct RenderArgs {
    #[arg(long, value_name = "PATH")]
    trajectory: PathBuf,
    #[arg(long, value_name = "PATH")]
    output: Option<PathBuf>,
}

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        eprintln!("error: {err:#}");
        process::exit(1);
    }
}

async fn run() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();
    match cli.command {
        Command::Case { command } => match command {
            CaseCommand::Init(args) => run_case_init(args),
        },
        Command::Evidence { command } => match command {
            EvidenceCommand::Add(args) => run_evidence_add(args),
        },
        Command::Change { command } => match command {
            ChangeCommand::Add(args) => run_change_add(args),
        },
        Command::Runbook { command } => match command {
            RunbookCommand::Add(args) => run_runbook_add(args),
        },
        Command::Investigate(args) => run_investigate(*args).await,
        Command::Config { command } => match command {
            ConfigCommand::Check(args) => run_config_check(args),
        },
        Command::Validate(args) => run_validate(args),
        Command::Render(args) => run_render(args),
        Command::Version => {
            println!("vigil {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
    }
}

async fn run_investigate(args: InvestigateArgs) -> Result<()> {
    check_requested_outputs(&[
        args.output.as_ref(),
        args.json_output.as_ref(),
        args.trajectory_output.as_ref(),
    ])?;

    let overrides = ConfigOverrides {
        account_id: args.account_id.clone(),
        api_token: args.api_token.clone(),
        gateway_id: args.gateway_id.clone(),
        model: args.model.clone(),
        endpoint: parse_endpoint_override(args.endpoint.as_deref())?,
        request_timeout_secs: args.request_timeout_secs,
        retry_count: args.retry_count,
        output_format: None,
    };
    let config = load_config(args.config.as_deref(), overrides)?;
    let provider = build_provider(&config, args.no_llm || args.dry_run || args.plan_only)?;
    let provider_ref = provider
        .as_ref()
        .map(|provider| provider as &dyn LlmProvider);

    if should_use_case_mode(&args)? {
        run_case_investigate(args, provider_ref).await
    } else if should_use_file_mode(&args) {
        run_file_investigate(args, provider_ref, config.output_format).await
    } else {
        run_agent_investigate(args, provider_ref, &config).await
    }
}

async fn run_file_investigate(
    args: InvestigateArgs,
    provider_ref: Option<&dyn LlmProvider>,
    output_format: OutputFormat,
) -> Result<()> {
    if args.plan_only {
        return Err(anyhow!(
            "--plan-only is only supported for target or alert investigation, for example: vigil investigate service:web --since 30m --plan-only"
        ));
    }
    if args.target.len() > 1 {
        return Err(anyhow!(
            "file-based investigation accepts at most one optional target selector"
        ));
    }
    let inventory = args.inventory.as_ref().ok_or_else(|| {
        anyhow!(
            "file-based investigation requires --inventory. To investigate a case, pass only the case directory, for example: vigil investigate web-5xx"
        )
    })?;
    check_readable_file(inventory)?;
    if let Some(alert) = &args.alert {
        check_readable_file(alert)?;
    }
    for runbook in &args.runbook {
        check_readable_file(runbook)?;
    }

    let outcome = investigate(
        InvestigationRequest {
            alert_path: args.alert.clone(),
            inventory_path: inventory.clone(),
            runbook_paths: args.runbook.clone(),
            runbook_dir: args.runbook_dir.clone(),
            target: args.target.first().cloned(),
            no_llm: args.no_llm,
            dry_run: args.dry_run,
        },
        provider_ref,
    )
    .await?;

    let markdown = render_markdown(&outcome.brief);
    let brief_json = render_json(&outcome.brief)?;
    let trajectory_json = render_trajectory_json(&outcome.trajectory)?;

    if let Some(path) = &args.output {
        write_output(path, &markdown)?;
    }
    if let Some(path) = &args.json_output {
        write_output(path, &brief_json)?;
    }
    if let Some(path) = &args.trajectory_output {
        write_output(path, &trajectory_json)?;
    }

    if args.output.is_none() && args.json_output.is_none() {
        match output_format {
            OutputFormat::Markdown => println!("{markdown}"),
            OutputFormat::Json => println!("{brief_json}"),
        }
    }

    Ok(())
}

async fn run_agent_investigate(
    args: InvestigateArgs,
    provider_ref: Option<&dyn LlmProvider>,
    config: &ResolvedConfig,
) -> Result<()> {
    let selector = parse_agent_selector(&args)?;
    let budget = investigation_budget(&args, config)?;
    let request = AgentInvestigationRequest {
        selector,
        since: args.since.clone(),
        sources: source_configs_from_resolved(config),
        source_filters: args.source.clone(),
        budget,
        no_llm: args.no_llm,
        dry_run: args.dry_run,
        plan_only: args.plan_only,
    };

    if args.plan_only {
        let outcome = plan_agent_investigation(request, provider_ref).await?;
        for warning in outcome.warnings {
            eprintln!("warning: {warning}");
        }
        println!("{}", render_tool_plan(&outcome.plan));
        return Ok(());
    }

    let outcome = investigate_agent(request, provider_ref).await?;
    let markdown = render_markdown(&outcome.brief);
    let brief_json = render_json(&outcome.brief)?;
    let trajectory_json = render_trajectory_json(&outcome.trajectory)?;

    let output_dir = PathBuf::from("output");
    if args.output.is_none() || args.json_output.is_none() || args.trajectory_output.is_none() {
        fs::create_dir_all(&output_dir).with_context(|| {
            format!(
                "agent output directory '{}' could not be created",
                output_dir.display()
            )
        })?;
    }
    let brief_path = args.output.unwrap_or_else(|| output_dir.join("brief.md"));
    let json_path = args
        .json_output
        .unwrap_or_else(|| output_dir.join("brief.json"));
    let trajectory_path = args
        .trajectory_output
        .unwrap_or_else(|| output_dir.join("trajectory.json"));

    write_output(&brief_path, &markdown)?;
    write_output(&json_path, &brief_json)?;
    write_output(&trajectory_path, &trajectory_json)?;
    println!("wrote {}", brief_path.display());
    println!("wrote {}", json_path.display());
    println!("wrote {}", trajectory_path.display());
    Ok(())
}

async fn run_case_investigate(
    args: InvestigateArgs,
    provider_ref: Option<&dyn LlmProvider>,
) -> Result<()> {
    let case_dir = args
        .target
        .first()
        .map(PathBuf::from)
        .ok_or_else(|| anyhow!("case investigation requires a case directory"))?;
    let output_dir = case_dir.join("output");
    fs::create_dir_all(&output_dir).with_context(|| {
        format!(
            "case output directory '{}' could not be created",
            output_dir.display()
        )
    })?;

    let outcome = investigate_case(
        CaseInvestigationRequest {
            case_dir: case_dir.clone(),
            no_llm: args.no_llm,
            dry_run: args.dry_run,
        },
        provider_ref,
    )
    .await?;

    let markdown = render_markdown(&outcome.brief);
    let brief_json = render_json(&outcome.brief)?;
    let trajectory_json = render_trajectory_json(&outcome.trajectory)?;
    let brief_path = args.output.unwrap_or_else(|| output_dir.join("brief.md"));
    let json_path = args
        .json_output
        .unwrap_or_else(|| output_dir.join("brief.json"));
    let trajectory_path = args
        .trajectory_output
        .unwrap_or_else(|| output_dir.join("trajectory.json"));

    write_output(&brief_path, &markdown)?;
    write_output(&json_path, &brief_json)?;
    write_output(&trajectory_path, &trajectory_json)?;
    println!("wrote {}", brief_path.display());
    println!("wrote {}", json_path.display());
    println!("wrote {}", trajectory_path.display());
    Ok(())
}

fn run_case_init(args: CaseInitArgs) -> Result<()> {
    let manifest = init_case(CaseInitRequest {
        case_dir: args.case_dir.clone(),
        target: args.target,
        severity: args.severity,
        summary: args.summary,
        force: args.force,
    })?;
    println!("created case {}", args.case_dir.display());
    println!("id: {}", manifest.id);
    Ok(())
}

fn run_evidence_add(args: EvidenceAddArgs) -> Result<()> {
    if let Some(file) = &args.file {
        check_readable_file(file)?;
    }
    let kind = EvidenceKind::from_str(&args.kind)?;
    let added = add_case_evidence(EvidenceAddRequest {
        case_dir: args.case_dir,
        kind,
        summary: args.summary,
        source: args.source,
        url: args.url,
        file: args.file,
    })?;
    println!("added evidence {}", added.path.display());
    Ok(())
}

fn run_change_add(args: ChangeAddArgs) -> Result<()> {
    let added = add_case_change(ChangeAddRequest {
        case_dir: args.case_dir,
        summary: args.summary,
        source: args.source,
        url: args.url,
    })?;
    println!("added change evidence {}", added.path.display());
    Ok(())
}

fn run_runbook_add(args: RunbookAddArgs) -> Result<()> {
    check_readable_file(&args.runbook)?;
    let path = add_case_runbook(RunbookAddRequest {
        case_dir: args.case_dir,
        runbook_path: args.runbook,
    })?;
    println!("added runbook {}", path.display());
    Ok(())
}

fn run_config_check(args: ConfigCheckArgs) -> Result<()> {
    let config = load_config(
        args.config.as_deref(),
        ConfigOverrides {
            account_id: args.account_id,
            api_token: args.api_token,
            gateway_id: args.gateway_id,
            model: args.model,
            endpoint: parse_endpoint_override(args.endpoint.as_deref())?,
            request_timeout_secs: args.request_timeout_secs,
            retry_count: args.retry_count,
            output_format: None,
        },
    )?;

    config.cloudflare.validate_for_llm()?;
    if let Some(path) = &args.alert {
        check_readable_file(path)?;
    }
    if let Some(path) = &args.inventory {
        check_readable_file(path)?;
    }
    if let Some(path) = &args.output {
        check_output_path(path)?;
    }

    println!("configuration ok");
    println!("{}", config.cloudflare.redacted_summary());
    Ok(())
}

fn run_validate(args: ValidateArgs) -> Result<()> {
    validate_input_files(ValidationRequest {
        alert_path: args.alert,
        inventory_path: args.inventory,
        runbook_paths: args.runbook,
        runbook_dir: args.runbook_dir,
    })?;
    println!("input validation ok");
    Ok(())
}

fn run_render(args: RenderArgs) -> Result<()> {
    if let Some(output) = &args.output {
        check_output_path(output)?;
    }
    let trajectory = load_trajectory(&args.trajectory)?;
    let markdown = render_markdown(&trajectory.brief);
    if let Some(path) = &args.output {
        write_output(path, &markdown)?;
    } else {
        println!("{markdown}");
    }
    Ok(())
}

fn check_requested_outputs(paths: &[Option<&PathBuf>]) -> Result<()> {
    for path in paths.iter().flatten() {
        check_output_path(path)?;
    }
    Ok(())
}

fn write_output(path: &Path, text: &str) -> Result<()> {
    fs::write(path, text)
        .with_context(|| format!("output file '{}' could not be written", path.display()))
}

fn required_setting(value: Option<String>, name: &'static str) -> Result<String> {
    value.with_context(|| format!("missing {name}; run 'vigil config check' for details"))
}

fn build_provider(
    config: &ResolvedConfig,
    skip_llm: bool,
) -> Result<Option<CloudflareAiGatewayProvider>> {
    if skip_llm {
        return Ok(None);
    }

    config.cloudflare.validate_for_llm()?;
    Ok(Some(CloudflareAiGatewayProvider::new(
        CloudflareAiGatewayConfig::new(
            required_setting(
                config.cloudflare.account_id.clone(),
                "Cloudflare account ID",
            )?,
            required_setting(config.cloudflare.api_token.clone(), "Cloudflare API token")?,
            required_setting(
                config.cloudflare.gateway_id.clone(),
                "Cloudflare AI Gateway ID",
            )?,
            config.cloudflare.model.clone(),
            config.cloudflare.request_timeout_secs,
            config.cloudflare.retry_count,
        )?
        .with_endpoint_style(match config.cloudflare.endpoint {
            CloudflareEndpoint::Rest => CloudflareEndpointStyle::Rest,
            CloudflareEndpoint::Gateway => CloudflareEndpointStyle::Gateway,
        }),
    )?))
}

fn parse_endpoint_override(value: Option<&str>) -> Result<Option<CloudflareEndpoint>> {
    value
        .map(|value| {
            CloudflareEndpoint::parse(value).ok_or_else(|| {
                anyhow!("invalid Cloudflare endpoint '{value}'. Use 'rest' or 'gateway'.")
            })
        })
        .transpose()
}

fn parse_agent_selector(args: &InvestigateArgs) -> Result<InvestigationSelector> {
    match args.target.as_slice() {
        [] => Err(anyhow!(
            "target or alert investigation requires a selector, for example: vigil investigate service:web --since 30m"
        )),
        [selector] => {
            if let Some(alert_name) = selector.strip_prefix("alert:") {
                if alert_name.trim().is_empty() {
                    Err(anyhow!("alert selector must include an alert name"))
                } else {
                    Ok(InvestigationSelector::Alert(alert_name.to_string()))
                }
            } else {
                Ok(InvestigationSelector::Target(selector.clone()))
            }
        }
        [kind, name] if kind == "alert" => {
            if name.trim().is_empty() {
                Err(anyhow!("alert selector must include an alert name"))
            } else {
                Ok(InvestigationSelector::Alert(name.clone()))
            }
        }
        [kind, _name] => Err(anyhow!(
            "unsupported investigation selector '{}'. Use 'alert NAME' or a target such as 'service:web'.",
            kind
        )),
        _ => Err(anyhow!("too many investigation selector arguments")),
    }
}

fn investigation_budget(
    args: &InvestigateArgs,
    config: &ResolvedConfig,
) -> Result<InvestigationBudget> {
    let budget = InvestigationBudget {
        max_iterations: args
            .max_iterations
            .unwrap_or(config.investigation.max_iterations),
        max_tool_calls: args
            .max_tool_calls
            .unwrap_or(config.investigation.max_tool_calls),
        max_duration_secs: args
            .max_duration_secs
            .unwrap_or(config.investigation.max_duration_secs),
    };

    if budget.max_iterations == 0 {
        return Err(anyhow!("max iterations must be greater than zero"));
    }
    if budget.max_tool_calls == 0 {
        return Err(anyhow!("max tool calls must be greater than zero"));
    }
    if budget.max_duration_secs == 0 {
        return Err(anyhow!("max duration must be greater than zero"));
    }
    Ok(budget)
}

fn source_configs_from_resolved(config: &ResolvedConfig) -> Vec<SourceConfig> {
    let mut sources = Vec::new();
    sources.extend(config.sources.inventory_files.iter().map(|source| {
        SourceConfig::InventoryFile {
            name: source.name.clone(),
            path: source.path.clone(),
        }
    }));
    sources.extend(
        config
            .sources
            .runbook_files
            .iter()
            .map(|source| SourceConfig::RunbookFile {
                name: source.name.clone(),
                dir: source.dir.clone(),
                paths: source.paths.clone(),
            }),
    );
    sources.extend(
        config
            .sources
            .alertmanagers
            .iter()
            .map(|source| SourceConfig::Alertmanager {
                name: source.name.clone(),
                url: source.url.clone(),
                fixture_path: source.fixture_path.clone(),
                bearer_token_env: source.bearer_token_env.clone(),
            }),
    );
    sources.extend(
        config
            .sources
            .prometheus
            .iter()
            .map(|source| SourceConfig::Prometheus {
                name: source.name.clone(),
                url: source.url.clone(),
                fixture_path: source.fixture_path.clone(),
                bearer_token_env: source.bearer_token_env.clone(),
            }),
    );
    sources.extend(
        config
            .sources
            .github
            .iter()
            .map(|source| SourceConfig::Github {
                name: source.name.clone(),
                api_url: source.api_url.clone(),
                repo: source.repo.clone(),
                fixture_path: source.fixture_path.clone(),
                bearer_token_env: source.bearer_token_env.clone(),
            }),
    );
    sources.extend(config.sources.http.iter().map(|source| SourceConfig::Http {
        name: source.name.clone(),
        url: source.url.clone(),
        fixture_path: source.fixture_path.clone(),
        bearer_token_env: source.bearer_token_env.clone(),
    }));
    sources.extend(config.sources.dns.iter().map(|source| SourceConfig::Dns {
        name: source.name.clone(),
        fixture_path: source.fixture_path.clone(),
    }));
    sources.extend(config.sources.loki.iter().map(|source| SourceConfig::Loki {
        name: source.name.clone(),
        url: source.url.clone(),
        fixture_path: source.fixture_path.clone(),
        bearer_token_env: source.bearer_token_env.clone(),
    }));
    sources.extend(
        config
            .sources
            .grafana
            .iter()
            .map(|source| SourceConfig::Grafana {
                name: source.name.clone(),
                url: source.url.clone(),
                fixture_path: source.fixture_path.clone(),
                bearer_token_env: source.bearer_token_env.clone(),
            }),
    );
    sources.extend(
        config
            .sources
            .kubernetes
            .iter()
            .map(|source| SourceConfig::Kubernetes {
                name: source.name.clone(),
                url: source.url.clone(),
                namespace: source.namespace.clone(),
                fixture_path: source.fixture_path.clone(),
                bearer_token_env: source.bearer_token_env.clone(),
            }),
    );
    sources
}

fn should_use_file_mode(args: &InvestigateArgs) -> bool {
    args.alert.is_some()
        || args.inventory.is_some()
        || !args.runbook.is_empty()
        || args.runbook_dir.is_some()
}

fn should_use_case_mode(args: &InvestigateArgs) -> Result<bool> {
    let has_file_mode_flags = should_use_file_mode(args);

    let Some(positional) = args.target.first() else {
        return Ok(false);
    };
    if args.target.len() > 1 {
        return Ok(false);
    }

    let positional_path = Path::new(positional);
    let positional_is_case = positional_path.join("vigil.yaml").exists();
    if positional_is_case && has_file_mode_flags {
        return Err(anyhow!(
            "ambiguous investigation input: '{}' is a case directory, but file-mode flags were also supplied. Use either 'vigil investigate {}' or the file-based --alert/--inventory workflow, not both.",
            positional,
            positional
        ));
    }

    Ok(positional_is_case && !has_file_mode_flags)
}
