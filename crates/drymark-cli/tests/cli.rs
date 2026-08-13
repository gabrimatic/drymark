#![allow(deprecated, missing_docs)]

use assert_cmd::Command;

fn command() -> Command {
    assert_cmd::cargo::cargo_bin_cmd!("drymark")
}

#[test]
fn clean_preserve_writes_only_sanitized_text() {
    command()
        .args(["clean", "--policy", "preserve"])
        .write_stdin("same\u{200b} words 👩‍💻")
        .assert()
        .success()
        .stdout("same words 👩‍💻")
        .stderr("");
}

#[test]
fn clean_thorough_canonicalizes_text() {
    command()
        .args(["clean", "--policy", "thorough"])
        .write_stdin("Cafe\u{301}\r\nA\u{00a0}B")
        .assert()
        .success()
        .stdout("Café\nA B")
        .stderr("");
}

#[test]
fn check_uses_dedicated_exit_three_when_cleaning_would_change_input() {
    command()
        .args(["clean", "--check"])
        .write_stdin("hidden\u{2060}mark")
        .assert()
        .code(3)
        .stdout("")
        .stderr("");
}

#[test]
fn usage_errors_remain_distinct_from_check_results() {
    command()
        .args(["clean", "--unknown-option"])
        .assert()
        .code(2)
        .stdout("");
}

#[test]
fn check_succeeds_when_input_is_already_clean() {
    command()
        .args(["clean", "--check"])
        .write_stdin("already clean\n")
        .assert()
        .success()
        .stdout("")
        .stderr("");
}

#[test]
fn scan_json_contains_metadata_but_never_input() -> Result<(), Box<dyn std::error::Error>> {
    let secret = "PRIVATE-ZXQ-9182\u{200b}";
    let output = command()
        .args(["scan", "--json"])
        .write_stdin(secret)
        .output()?;

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(json["changed"], true);
    assert_eq!(json["removed"][0]["category"], "zero_width");
    let rendered = String::from_utf8(output.stdout)?;
    assert!(!rendered.contains("PRIVATE"));
    assert!(!rendered.contains("ZXQ-9182"));
    Ok(())
}

#[test]
fn invalid_utf8_fails_without_echoing_input_bytes() {
    command()
        .args(["clean"])
        .write_stdin(vec![0xff, 0xfe, b'X'])
        .assert()
        .code(1)
        .stdout("")
        .stderr("drymark: input is not valid UTF-8\n");
}

#[test]
fn exact_input_limit_is_accepted_without_output_in_check_mode() {
    command()
        .args(["clean", "--check"])
        .write_stdin(vec![b'a'; 16 * 1024 * 1024])
        .assert()
        .success()
        .stdout("")
        .stderr("");
}

#[test]
fn one_byte_over_input_limit_is_rejected_without_echoing_input() {
    command()
        .args(["clean", "--check"])
        .write_stdin(vec![b'Z'; 16 * 1024 * 1024 + 1])
        .assert()
        .code(1)
        .stdout("")
        .stderr("drymark: input exceeds the 16 MiB safety limit\n");
}

#[test]
fn version_is_stable_and_product_named() {
    command()
        .arg("--version")
        .assert()
        .success()
        .stdout("drymark 0.1.0\n")
        .stderr("");
}
