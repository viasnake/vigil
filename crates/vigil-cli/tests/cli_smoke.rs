use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn unique_output_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let millis = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
    let dir = std::env::temp_dir().join(format!("vigil-cli-test-{millis}"));
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

#[test]
fn version_command_works() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_vigil"))
        .arg("version")
        .output()?;

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains("vigil "));
    Ok(())
}

#[test]
fn investigate_minimal_example_no_llm_writes_outputs() -> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root();
    let out_dir = unique_output_dir()?;
    let brief = out_dir.join("brief.md");
    let json = out_dir.join("brief.json");
    let trajectory = out_dir.join("trajectory.json");

    let output = Command::new(env!("CARGO_BIN_EXE_vigil"))
        .current_dir(&root)
        .args([
            "investigate",
            "--alert",
            "examples/minimal/alert.yaml",
            "--inventory",
            "examples/minimal/inventory.yaml",
            "--runbook-dir",
            "examples/minimal/runbooks",
            "--output",
            brief.to_string_lossy().as_ref(),
            "--json-output",
            json.to_string_lossy().as_ref(),
            "--trajectory-output",
            trajectory.to_string_lossy().as_ref(),
            "--no-llm",
        ])
        .output()?;

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8(output.stderr)?
    );
    let brief_text = fs::read_to_string(&brief)?;
    let json_text = fs::read_to_string(&json)?;
    let trajectory_text = fs::read_to_string(&trajectory)?;

    assert!(brief_text.contains("Investigation Brief: web"));
    assert!(brief_text.contains("Recommended Read-Only Checks"));
    let brief_value: serde_json::Value = serde_json::from_str(&json_text)?;
    let trajectory_value: serde_json::Value = serde_json::from_str(&trajectory_text)?;
    assert_eq!(brief_value["title"], "Investigation Brief: web");
    assert_eq!(
        trajectory_value["brief"]["title"],
        "Investigation Brief: web"
    );
    Ok(())
}

#[test]
fn render_from_trajectory_writes_markdown() -> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root();
    let out_dir = unique_output_dir()?;
    let trajectory = out_dir.join("trajectory.json");
    let rendered = out_dir.join("rendered.md");

    let investigate = Command::new(env!("CARGO_BIN_EXE_vigil"))
        .current_dir(&root)
        .args([
            "investigate",
            "--alert",
            "examples/minimal/alert.yaml",
            "--inventory",
            "examples/minimal/inventory.yaml",
            "--runbook-dir",
            "examples/minimal/runbooks",
            "--trajectory-output",
            trajectory.to_string_lossy().as_ref(),
            "--no-llm",
        ])
        .output()?;
    assert!(
        investigate.status.success(),
        "stderr: {}",
        String::from_utf8(investigate.stderr)?
    );

    let render = Command::new(env!("CARGO_BIN_EXE_vigil"))
        .current_dir(&root)
        .args([
            "render",
            "--trajectory",
            trajectory.to_string_lossy().as_ref(),
            "--output",
            rendered.to_string_lossy().as_ref(),
        ])
        .output()?;

    assert!(
        render.status.success(),
        "stderr: {}",
        String::from_utf8(render.stderr)?
    );
    let markdown = fs::read_to_string(&rendered)?;
    assert!(markdown.contains("Investigation Brief: web"));
    Ok(())
}
