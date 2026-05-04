use serde_json::Value;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};
use tempfile::TempDir;

const PASSWORD: &str = "correct horse battery staple";

fn lockit() -> Command {
    Command::new(env!("CARGO_BIN_EXE_lockit"))
}

fn run_lockit(vault: &Path, args: &[&str]) -> Output {
    let output = lockit()
        .arg("--vault")
        .arg(vault)
        .arg("--password")
        .arg(PASSWORD)
        .args(args)
        .output()
        .expect("run lockit");
    assert_success(&output, args);
    output
}

fn run_lockit_with_stdin(vault: &Path, args: &[&str], input: &str) -> Output {
    let mut child = lockit()
        .arg("--vault")
        .arg(vault)
        .arg("--password")
        .arg(PASSWORD)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn lockit");

    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(input.as_bytes())
        .expect("write stdin");

    let output = child.wait_with_output().expect("wait for lockit");
    assert_success(&output, args);
    output
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

fn assert_success(output: &Output, args: &[&str]) {
    assert!(
        output.status.success(),
        "lockit {args:?} failed\nstdout:\n{}\nstderr:\n{}",
        stdout(output),
        stderr(output)
    );
}

fn credentials_json(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).expect("valid JSON output")
}

#[test]
fn manages_a_vault_through_non_interactive_cli_commands() {
    let temp = TempDir::new().expect("tempdir");
    let vault = temp.path().join("vault.enc");

    let init = run_lockit(&vault, &["init"]);
    assert!(stdout(&init).contains("Vault initialized"));

    let input = r#"{
        "type": "api_key",
        "name": "OPENAI_API_KEY",
        "service": "openai",
        "key": "production",
        "fields": {
            "secret_value": "sk-test-secret",
            "region": "us-east-1"
        }
    }"#;
    let add = run_lockit_with_stdin(&vault, &["add", "--stdin"], input);
    assert!(stdout(&add).contains("Credential added:"));

    let list = run_lockit(&vault, &["list", "--json"]);
    let list_json = credentials_json(&list);
    let credentials = list_json["credentials"]
        .as_array()
        .expect("credentials array");
    assert_eq!(credentials.len(), 1);
    assert_eq!(credentials[0]["name"], "OPENAI_API_KEY");
    assert_eq!(credentials[0]["type"], "api_key");
    assert_eq!(credentials[0]["service"], "openai");
    assert_eq!(credentials[0]["key"], "production");
    assert_ne!(credentials[0]["fields"]["secret_value"], "sk-test-secret");

    let searched = run_lockit(&vault, &["list", "--json", "openai"]);
    assert_eq!(
        credentials_json(&searched)["credentials"]
            .as_array()
            .expect("credentials array")
            .len(),
        1
    );

    let show = run_lockit(&vault, &["show", "OPENAI_API_KEY", "--json"]);
    assert!(stdout(&show).contains("OPENAI_API_KEY"));
    assert!(!stdout(&show).contains("sk-test-secret"));

    let reveal = run_lockit(&vault, &["reveal", "OPENAI_API_KEY", "secret_value"]);
    assert_eq!(stdout(&reveal).trim(), "sk-test-secret");

    let env = run_lockit(&vault, &["env", "OPENAI_API_KEY"]);
    assert!(stdout(&env).contains("export OPENAI_API_KEY_SECRET_VALUE='sk-test-secret'"));
    assert!(stdout(&env).contains("export OPENAI_API_KEY_REGION='us-east-1'"));

    let injected = run_lockit(
        &vault,
        &[
            "run",
            "OPENAI_API_KEY",
            "--",
            "sh",
            "-c",
            "printf '%s' \"$OPENAI_API_KEY_SECRET_VALUE\"",
        ],
    );
    assert_eq!(stdout(&injected), "sk-test-secret");

    let export = run_lockit(&vault, &["export", "--json"]);
    assert!(stdout(&export).contains("OPENAI_API_KEY"));
    assert!(stdout(&export).contains("sk-test-secret"));

    let delete = run_lockit(&vault, &["delete", "OPENAI_API_KEY"]);
    assert!(stdout(&delete).contains("Deleted: OPENAI_API_KEY"));

    let empty = run_lockit(&vault, &["list", "--json"]);
    assert!(credentials_json(&empty)["credentials"]
        .as_array()
        .expect("credentials array")
        .is_empty());
}

#[test]
fn adds_from_file_and_imports_json_backup_arrays() {
    let temp = TempDir::new().expect("tempdir");
    let vault = temp.path().join("vault.enc");
    run_lockit(&vault, &["init"]);

    let credential_file = temp.path().join("credential.json");
    fs::write(
        &credential_file,
        r#"{
            "type": "database_url",
            "name": "POSTGRES_PROD",
            "service": "postgres",
            "key": "primary",
            "fields": {
                "connection_url": "postgres://user:pass@example.test/db"
            }
        }"#,
    )
    .expect("write credential file");

    run_lockit(
        &vault,
        &[
            "add",
            "--file",
            credential_file.to_str().expect("utf8 path"),
        ],
    );
    let database_url = run_lockit(&vault, &["reveal", "POSTGRES_PROD", "connection_url"]);
    assert_eq!(
        stdout(&database_url).trim(),
        "postgres://user:pass@example.test/db"
    );

    let import_file = temp.path().join("backup.json");
    fs::write(
        &import_file,
        r#"[{
            "type": "token",
            "name": "DEPLOY_TOKEN",
            "service": "deploy",
            "key": "ci",
            "fields": {
                "token_value": "tok-123"
            }
        }]"#,
    )
    .expect("write import file");

    let imported = run_lockit(
        &vault,
        &["import", import_file.to_str().expect("utf8 path")],
    );
    assert!(stdout(&imported).contains("Imported 1 credentials"));

    let token = run_lockit(&vault, &["reveal", "DEPLOY_TOKEN", "token_value"]);
    assert_eq!(stdout(&token).trim(), "tok-123");

    let list = run_lockit(&vault, &["list", "--json"]);
    assert_eq!(
        credentials_json(&list)["credentials"]
            .as_array()
            .expect("credentials array")
            .len(),
        2
    );
}

#[test]
fn reports_error_when_vault_is_not_initialized() {
    let temp = TempDir::new().expect("tempdir");
    let vault = temp.path().join("missing-vault.enc");

    let output = lockit()
        .arg("--vault")
        .arg(&vault)
        .arg("--password")
        .arg(PASSWORD)
        .args(["list", "--json"])
        .output()
        .expect("run lockit");

    assert!(!output.status.success());
    assert!(stderr(&output).contains("vault is not initialized"));
}
