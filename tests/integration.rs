use assert_cmd::Command;
use ooxml_version_control::filesystem;
use ooxml_version_control::ooxml::{read_xml_file, schemas::shared_strings};
use std::fs;
use std::path::PathBuf;
use std::process::Command as ProcessCommand;
use tempfile::tempdir;

#[test]
fn test_check_in_with_valid_file() {
    let fixture = PathBuf::from("tests/fixtures/simple_book.xlsx");
    let mut cmd = Command::cargo_bin("ooxml-version-control").unwrap();
    let test_file_path_1: PathBuf;
    let test_file_path_2: PathBuf;
    let output_dir_1: PathBuf;
    let output_dir_2: PathBuf;

    {
        let temp_dir = tempdir().unwrap();
        test_file_path_1 = temp_dir.path().join("simple_book_1.xlsx");
        test_file_path_2 = temp_dir.path().join("simple_book_2.xlsx");

        fs::copy(&fixture, &test_file_path_1).unwrap();
        fs::copy(&fixture, &test_file_path_2).unwrap();

        cmd.arg("check-in")
            .arg(&test_file_path_1)
            .arg(&test_file_path_2)
            .assert()
            .success()
            .stderr(predicates::str::contains("Checking in"));

        output_dir_1 = test_file_path_1.with_file_name("simple_book_1.xlsx_ooxml");
        output_dir_2 = test_file_path_1.with_file_name("simple_book_2.xlsx_ooxml");

        assert_eq!(output_dir_1.is_dir(), true);
        assert_eq!(output_dir_2.is_dir(), true);

        let expected_files = vec![
            "docProps/core.xml",
            "xl/workbook.xml",
            "xl/worksheets/sheet1.xml",
        ];

        for expected_file in &expected_files {
            let path = output_dir_1.join(expected_file);
            assert_eq!(path.is_file(), true);
            let path = output_dir_2.join(expected_file);
            assert_eq!(path.is_file(), true);
        }

        let calc_chain_path_1 = output_dir_1.join("xl/calcChain.xml");
        assert!(!calc_chain_path_1.exists());

        let calc_chain_path_2 = output_dir_2.join("xl/calcChain.xml");
        assert!(!calc_chain_path_2.exists());

        let workbook_rels_1 = fs::read_to_string(output_dir_1.join("xl/_rels/workbook.xml.rels"))
            .expect("workbook rels should be readable");
        let workbook_rels_2 = fs::read_to_string(output_dir_2.join("xl/_rels/workbook.xml.rels"))
            .expect("workbook rels should be readable");
        assert!(!workbook_rels_1.contains("calcChain"));
        assert!(!workbook_rels_2.contains("calcChain"));

        let content_types_1 = fs::read_to_string(output_dir_1.join("[Content_Types].xml"))
            .expect("content types should be readable");
        let content_types_2 = fs::read_to_string(output_dir_2.join("[Content_Types].xml"))
            .expect("content types should be readable");
        assert!(!content_types_1.contains("/xl/calcChain.xml"));
        assert!(!content_types_2.contains("/xl/calcChain.xml"));

        let core_props_1 = fs::read_to_string(output_dir_1.join("docProps/core.xml"))
            .expect("core props readable");
        let core_props_2 = fs::read_to_string(output_dir_2.join("docProps/core.xml"))
            .expect("core props readable");
        for core_props in [&core_props_1, &core_props_2] {
            assert!(!core_props.contains("lastModifiedBy"));
            assert!(!core_props.contains("<cp:revision>"));
            assert!(!core_props.contains("dcterms:modified"));
        }

        let app_props_1 =
            fs::read_to_string(output_dir_1.join("docProps/app.xml")).expect("app props readable");
        let app_props_2 =
            fs::read_to_string(output_dir_2.join("docProps/app.xml")).expect("app props readable");
        for app_props in [&app_props_1, &app_props_2] {
            assert!(!app_props.contains("<TotalTime>"));
            assert!(!app_props.contains("<AppVersion>"));
        }
    }

    assert_ne!(output_dir_1.is_dir(), true);
    assert_ne!(output_dir_2.is_dir(), true);
}

#[test]
fn test_check_in_with_invalid_file() {
    let mut cmd = Command::cargo_bin("ooxml-version-control").unwrap();

    let invalid_file = PathBuf::from("tests/fixtures/non_existent_file.xlsx");

    cmd.arg("check-in")
        .arg(&invalid_file)
        .assert()
        .failure()
        .stderr(predicates::str::contains("Error: Path is not a valid file"));
}

#[test]
fn test_check_out_with_valid_file() {
    let fixture = PathBuf::from("tests/fixtures/simple_book.xlsx_ooxml");
    let mut cmd = Command::cargo_bin("ooxml-version-control").unwrap();
    let test_folder_path: PathBuf;
    let output_file: PathBuf;

    {
        let temp_dir = tempdir().unwrap();
        test_folder_path = temp_dir.path().join("simple_book.xlsx_ooxml");
        filesystem::copy_dir(&fixture, &test_folder_path);

        cmd.arg("check-out")
            .arg(&test_folder_path)
            .assert()
            .success()
            .stderr(predicates::str::contains("Checking out"));

        output_file = test_folder_path.with_file_name("simple_book.xlsx");

        assert_eq!(output_file.is_file(), true);
    }

    assert_ne!(output_file.is_file(), true);
}

#[test]
fn test_check_out_with_invalid_file() {
    let mut cmd = Command::cargo_bin("ooxml-version-control").unwrap();

    let invalid_folder = PathBuf::from("tests/fixtures/non_existent_folder.xlsx_ooxml");

    cmd.arg("check-out")
        .arg(invalid_folder)
        .assert()
        .failure()
        .stderr(predicates::str::contains("Error: Path is not a valid file"));
}

#[test]
fn test_read_xml_file_from_integration_target() {
    let sst: shared_strings::Sst =
        read_xml_file("tests/fixtures/simple_book.xlsx_ooxml/xl/sharedStrings.xml").unwrap();

    assert_eq!(sst.count, "2");
    assert_eq!(sst.unique_count, "2");
    assert_eq!(sst.si.len(), 2);
}

#[test]
fn test_vcs_in_with_valid_file() {
    let fixture = PathBuf::from("tests/fixtures/simple_book.xlsx");
    let mut cmd = Command::cargo_bin("ooxml-version-control").unwrap();
    let test_file_path: PathBuf;
    let output_dir: PathBuf;

    {
        let temp_dir = tempdir().unwrap();
        test_file_path = temp_dir.path().join("simple_book.xlsx");
        fs::copy(&fixture, &test_file_path).unwrap();

        cmd.arg("vcs-in")
            .arg(&test_file_path)
            .assert()
            .success()
            .stderr(predicates::str::contains("Checking in"));

        output_dir = test_file_path.with_file_name("simple_book.xlsx_ooxml");
        assert!(output_dir.is_dir());
    }

    assert!(!output_dir.is_dir());
}

#[test]
fn test_vcs_out_with_valid_dir() {
    let fixture = PathBuf::from("tests/fixtures/simple_book.xlsx_ooxml");
    let mut cmd = Command::cargo_bin("ooxml-version-control").unwrap();
    let test_folder_path: PathBuf;
    let output_file: PathBuf;

    {
        let temp_dir = tempdir().unwrap();
        test_folder_path = temp_dir.path().join("simple_book.xlsx_ooxml");
        filesystem::copy_dir(&fixture, &test_folder_path);

        cmd.arg("vcs-out")
            .arg(&test_folder_path)
            .assert()
            .success()
            .stderr(predicates::str::contains("Checking out"));

        output_file = test_folder_path.with_file_name("simple_book.xlsx");
        assert!(output_file.is_file());
    }

    assert!(!output_file.is_file());
}

#[test]
fn test_git_install_creates_hooks() {
    let temp_dir = tempdir().unwrap();
    let repo = temp_dir.path().to_path_buf();

    let init_status = ProcessCommand::new("git")
        .arg("init")
        .arg(&repo)
        .status()
        .unwrap();
    assert!(init_status.success());

    let mut cmd = Command::cargo_bin("ooxml-version-control").unwrap();
    cmd.arg("git-install")
        .arg("--repo")
        .arg(&repo)
        .assert()
        .success();

    assert!(repo.join(".git/hooks/pre-commit").is_file());
    assert!(repo.join(".git/hooks/post-checkout").is_file());
    assert!(repo.join(".git/hooks/post-merge").is_file());

    let pre_commit = fs::read_to_string(repo.join(".git/hooks/pre-commit")).unwrap();
    let post_checkout = fs::read_to_string(repo.join(".git/hooks/post-checkout")).unwrap();
    let post_merge = fs::read_to_string(repo.join(".git/hooks/post-merge")).unwrap();
    assert!(pre_commit.contains("vcs-in --stage"));
    assert!(post_checkout.contains("vcs-out"));
    assert!(post_merge.contains("vcs-out"));
}
