use assert_cmd::Command;
use predicates::prelude::*;

#[allow(deprecated)]
fn toy_qc() -> Command {
    Command::cargo_bin("toy-qc").unwrap()
}

// --- Parse tests ---

#[test]
fn test_parse_bell_pair() {
    toy_qc()
        .args(["parse", "programs/bell_pair.kirin"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .assert()
        .success()
        .stdout(predicate::str::contains("h %q0"))
        .stdout(predicate::str::contains("cnot %q0_h"));
}

#[test]
fn test_parse_ghz() {
    toy_qc()
        .args(["parse", "programs/ghz.kirin"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .assert()
        .success()
        .stdout(predicate::str::contains("h %q0"))
        .stdout(predicate::str::contains("cnot %q0_h"));
}

#[test]
fn test_parse_rz_circuit() {
    toy_qc()
        .args(["parse", "programs/rz_circuit.kirin"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .assert()
        .success()
        .stdout(predicate::str::contains("rz"))
        .stdout(predicate::str::contains("1.5708"))
        .stdout(predicate::str::contains("h %q1"));
}

#[test]
fn test_parse_bell_pair_zx() {
    toy_qc()
        .args(["parse", "programs/bell_pair_zx.kirin"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .assert()
        .success()
        .stdout(predicate::str::contains("wire"))
        .stdout(predicate::str::contains("z_spider"))
        .stdout(predicate::str::contains("h_box"));
}

// --- Roundtrip tests ---

fn roundtrip(program: &str) {
    // First parse
    let first = toy_qc()
        .args(["parse", &format!("programs/{program}")])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("failed to run toy-qc");
    assert!(first.status.success(), "first parse failed");
    let first_output = String::from_utf8(first.stdout).unwrap();

    // Write to temp file and re-parse
    let tmp = std::env::temp_dir().join(format!("toy_qc_roundtrip_{program}"));
    std::fs::write(&tmp, &first_output).unwrap();

    let second = toy_qc()
        .args(["parse", tmp.to_str().unwrap()])
        .output()
        .expect("failed to run toy-qc");
    assert!(second.status.success(), "second parse failed");
    let second_output = String::from_utf8(second.stdout).unwrap();

    assert_eq!(
        first_output, second_output,
        "roundtrip mismatch for {program}"
    );

    // Cleanup
    let _ = std::fs::remove_file(&tmp);
}

#[test]
fn test_roundtrip_bell_pair() {
    roundtrip("bell_pair.kirin");
}

#[test]
fn test_roundtrip_ghz() {
    roundtrip("ghz.kirin");
}

#[test]
fn test_roundtrip_rz_circuit() {
    roundtrip("rz_circuit.kirin");
}

#[test]
fn test_roundtrip_bell_pair_zx() {
    roundtrip("bell_pair_zx.kirin");
}

// --- Error case ---

#[test]
fn test_parse_missing_file() {
    toy_qc()
        .args(["parse", "programs/nonexistent.kirin"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .assert()
        .failure();
}
