use assert_cmd::cargo::cargo_bin_cmd;
use tempfile::tempdir;

#[test]
fn add_and_list_event() {
    let data_home = tempdir().expect("temp dir");

    cargo_bin_cmd!("toki-note")
        .env("XDG_DATA_HOME", data_home.path())
        .arg("add")
        .arg("--title")
        .arg("テスト予定")
        .arg("--start")
        .arg("2025-12-01T09:00:00+09:00")
        .arg("--duration")
        .arg("30m")
        .assert()
        .success();

    let output = cargo_bin_cmd!("toki-note")
        .env("XDG_DATA_HOME", data_home.path())
        .arg("list")
        .output()
        .expect("run list");

    assert!(
        output.status.success(),
        "list command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("テスト予定"),
        "expected list output to mention title, got:\n{stdout}"
    );
}
