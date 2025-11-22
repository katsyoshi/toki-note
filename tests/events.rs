use assert_cmd::Command;

#[test]
fn add_and_list_event() {
    let mut cmd = Command::cargo_bin("toki-note").unwrap();
    cmd.arg("add")
        .arg("--title")
        .arg("テスト予定")
        .arg("--start")
        .arg("2025-12-01T09:00:00+09:00")
        .arg("--duration")
        .arg("30m")
        .assert()
        .success();

    let mut list = Command::cargo_bin("toki-note").unwrap();
    list.arg("list")
        .assert()
        .success();
}
