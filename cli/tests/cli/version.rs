use std::process::Command;

#[test]
fn version_flag_prints_binary_name_and_version() {
    let out = Command::new(env!("CARGO_BIN_EXE_agentgate"))
        .arg("--version")
        .output()
        .unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.starts_with("agentgate "), "stdout: {stdout}");
    assert!(stdout.contains(agentgate_core::VERSION), "stdout: {stdout}");
}
