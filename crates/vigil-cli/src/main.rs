use std::{fs, path::PathBuf, process};

use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};
use vigil_config::{
    check_output_path, check_readable_file, load_config, ConfigOverrides, OutputFormat,
};
use vigil_core::{
    investigate, load_trajectory, validate_input_files, InvestigationRequest, ValidationRequest,
};
use vigil_llm::{CloudflareAiGatewayConfig, CloudflareAiGatewayProvider, LlmProvider};
use vigil_render::{render_json, render_markdown, render_trajectory_json};

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
    Investigate(InvestigateArgs),
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
    Validate(ValidateArgs),
    Render(RenderArgs),
    Version,
}

#[derive(Debug, Args)]
struct InvestigateArgs {
    #[arg(long, value_name = "PATH")]
    alert: Option<PathBuf>,
    #[arg(long, value_name = "PATH")]
    inventory: PathBuf,
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
    #[arg(value_name = "TARGET")]
    target: Option<String>,
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
        Command::Investigate(args) => run_investigate(args).await,
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
    check_readable_file(&args.inventory)?;
    if let Some(alert) = &args.alert {
        check_readable_file(alert)?;
    }
    for runbook in &args.runbook {
        check_readable_file(runbook)?;
    }
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
        request_timeout_secs: args.request_timeout_secs,
        retry_count: args.retry_count,
        output_format: None,
    };
    let config = load_config(args.config.as_deref(), overrides)?;

    let provider = if args.no_llm || args.dry_run {
        None
    } else {
        config.cloudflare.validate_for_llm()?;
        Some(CloudflareAiGatewayProvider::new(
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
            )?,
        )?)
    };
    let provider_ref = provider
        .as_ref()
        .map(|provider| provider as &dyn LlmProvider);

    let outcome = investigate(
        InvestigationRequest {
            alert_path: args.alert.clone(),
            inventory_path: args.inventory.clone(),
            runbook_paths: args.runbook.clone(),
            runbook_dir: args.runbook_dir.clone(),
            target: args.target.clone(),
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
        match config.output_format {
            OutputFormat::Markdown => println!("{markdown}"),
            OutputFormat::Json => println!("{brief_json}"),
        }
    }

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

fn write_output(path: &PathBuf, text: &str) -> Result<()> {
    fs::write(path, text)
        .with_context(|| format!("output file '{}' could not be written", path.display()))
}

fn required_setting(value: Option<String>, name: &'static str) -> Result<String> {
    value.with_context(|| format!("missing {name}; run 'vigil config check' for details"))
}
