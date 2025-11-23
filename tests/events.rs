use assert_cmd::cargo::cargo_bin_cmd;
use tempfile::tempdir;

fn parse_row_id(output: &[u8]) -> i64 {
    String::from_utf8_lossy(output)
        .lines()
        .find_map(|line| line.trim().strip_prefix("Stored event #"))
        .and_then(|v| v.trim().parse().ok())
        .expect("row id in output")
}

#[test]
fn add_and_list_event() {
    let data_home = tempdir().expect("temp dir");

    cargo_bin_cmd!("toki-note")
        .env("XDG_DATA_HOME", data_home.path())
        .arg("add")
        .arg("--title")
        .arg("テスト予定")
        .arg("--date")
        .arg("2025-12-01")
        .arg("--time")
        .arg("09:00")
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

#[test]
fn ls_alias_works() {
    let data_home = tempdir().expect("temp dir");

    cargo_bin_cmd!("toki-note")
        .env("XDG_DATA_HOME", data_home.path())
        .arg("add")
        .arg("--title")
        .arg("Alias test")
        .arg("--date")
        .arg("2025-12-02")
        .arg("--time")
        .arg("10:00")
        .assert()
        .success();

    cargo_bin_cmd!("toki-note")
        .env("XDG_DATA_HOME", data_home.path())
        .arg("ls")
        .assert()
        .success();
}

#[test]
fn rm_alias_works() {
    let data_home = tempdir().expect("temp dir");

    cargo_bin_cmd!("toki-note")
        .env("XDG_DATA_HOME", data_home.path())
        .arg("add")
        .arg("--title")
        .arg("Remove me")
        .arg("--date")
        .arg("2025-12-03")
        .arg("--time")
        .arg("15:00")
        .assert()
        .success();

    cargo_bin_cmd!("toki-note")
        .env("XDG_DATA_HOME", data_home.path())
        .arg("rm")
        .arg("--title")
        .arg("Remove me")
        .assert()
        .success();
}

#[test]
fn move_command_updates_event() {
    let data_home = tempdir().expect("temp dir");

    let add_output = cargo_bin_cmd!("toki-note")
        .env("XDG_DATA_HOME", data_home.path())
        .arg("add")
        .arg("--title")
        .arg("Move me")
        .arg("--date")
        .arg("2025-12-04")
        .arg("--time")
        .arg("08:00")
        .output()
        .expect("add output");
    assert!(add_output.status.success());
    let id = parse_row_id(&add_output.stdout);

    cargo_bin_cmd!("toki-note")
        .env("XDG_DATA_HOME", data_home.path())
        .arg("move")
        .arg("--id")
        .arg(id.to_string())
        .arg("--start")
        .arg("2025-12-05T14:45:00+00:00")
        .assert()
        .success();

    let list = cargo_bin_cmd!("toki-note")
        .env("XDG_DATA_HOME", data_home.path())
        .arg("list")
        .arg("--tz")
        .arg("UTC")
        .output()
        .expect("list after move");
    assert!(list.status.success());
    let stdout = String::from_utf8_lossy(&list.stdout);
    assert!(
        stdout.contains("2025-12-05 14:45 UTC"),
        "expected moved time, got:\n{stdout}"
    );
}
