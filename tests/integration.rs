use assert_cmd::Command;
use std::path::PathBuf;

#[test]
fn test_check_in_with_valid_file() {
    let mut cmd = Command::cargo_bin("ooxml-version-control").unwrap();
    let test_file = PathBuf::from("tests/fixtures/simple_book.xlsx");

    cmd.arg("check-in")
        .arg(test_file)
        .assert()
        .success()
        .stdout(predicates::str::contains("Checking in"));
}

#[test]
fn test_check_in_with_invalid_file() {
    let mut cmd = Command::cargo_bin("ooxml-version-control").unwrap();
    let invalid_file = PathBuf::from("tests/fixtures/non_existent_file.xlsx");

    cmd.arg("check-in")
        .arg(invalid_file)
        .assert()
        .failure()
        .stderr(predicates::str::contains("Error: Path is not a valid file"));
}

#[test]
fn test_check_out_with_valid_file() {
    let mut cmd = Command::cargo_bin("ooxml-version-control").unwrap();
    let test_file = PathBuf::from("tests/fixtures/simple_book.xlsx");

    cmd.arg("check-out")
        .arg(test_file)
        .assert()
        .success()
        .stdout(predicates::str::contains("Checking out"));
}

#[test]
fn test_check_out_with_invalid_file() {
    let mut cmd = Command::cargo_bin("ooxml-version-control").unwrap();
    let invalid_file = PathBuf::from("tests/fixtures/non_existent_file.xlsx");

    cmd.arg("check-out")
        .arg(invalid_file)
        .assert()
        .failure()
        .stderr(predicates::str::contains("Error: Path is not a valid file"));
}
