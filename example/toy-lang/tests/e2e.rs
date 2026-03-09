use assert_cmd::Command;
use predicates::prelude::*;

#[allow(deprecated)]
fn toy_lang() -> Command {
    Command::cargo_bin("toy-lang").unwrap()
}

#[test]
fn test_parse_add() {
    toy_lang()
        .args(["parse", "programs/add.kirin"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .assert()
        .success()
        .stdout(predicate::str::contains("add"));
}

#[test]
fn test_run_add() {
    toy_lang()
        .args([
            "run",
            "programs/add.kirin",
            "--stage",
            "source",
            "--function",
            "main",
            "3",
            "5",
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .assert()
        .success()
        .stdout("8\n");
}

#[test]
fn test_run_factorial_5() {
    toy_lang()
        .args([
            "run",
            "programs/factorial.kirin",
            "--stage",
            "source",
            "--function",
            "factorial",
            "5",
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .assert()
        .success()
        .stdout("120\n");
}

#[test]
fn test_run_factorial_0() {
    toy_lang()
        .args([
            "run",
            "programs/factorial.kirin",
            "--stage",
            "source",
            "--function",
            "factorial",
            "0",
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .assert()
        .success()
        .stdout("1\n");
}

#[test]
fn test_run_factorial_1() {
    toy_lang()
        .args([
            "run",
            "programs/factorial.kirin",
            "--stage",
            "source",
            "--function",
            "factorial",
            "1",
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .assert()
        .success()
        .stdout("1\n");
}

#[test]
fn test_run_abs_positive() {
    toy_lang()
        .args([
            "run",
            "programs/branching.kirin",
            "--stage",
            "source",
            "--function",
            "abs",
            "42",
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .assert()
        .success()
        .stdout("42\n");
}

#[test]
fn test_run_abs_negative() {
    toy_lang()
        .args([
            "run",
            "programs/branching.kirin",
            "--stage",
            "source",
            "--function",
            "abs",
            "--",
            "-7",
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .assert()
        .success()
        .stdout("7\n");
}

#[test]
fn test_run_abs_zero() {
    toy_lang()
        .args([
            "run",
            "programs/branching.kirin",
            "--stage",
            "source",
            "--function",
            "abs",
            "0",
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .assert()
        .success()
        .stdout("0\n");
}

#[test]
fn test_run_missing_function() {
    toy_lang()
        .args([
            "run",
            "programs/add.kirin",
            "--stage",
            "source",
            "--function",
            "nonexistent",
            "1",
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .assert()
        .failure();
}

#[test]
fn test_run_missing_stage() {
    toy_lang()
        .args([
            "run",
            "programs/add.kirin",
            "--stage",
            "nonexistent",
            "--function",
            "main",
            "1",
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .assert()
        .failure();
}
