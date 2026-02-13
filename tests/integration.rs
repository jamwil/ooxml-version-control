use assert_cmd::Command;
use ooxml_version_control::filesystem;
use std::fs;
use std::path::PathBuf;
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
        assert!(
            !calc_chain_path_1.exists(),
            "calcChain.xml should not exist in output 1"
        );

        let calc_chain_path_2 = output_dir_2.join("xl/calcChain.xml");
        assert!(
            !calc_chain_path_2.exists(),
            "calcChain.xml should not exist in output 2"
        );

        let workbook_rels_1 = fs::read_to_string(output_dir_1.join("xl/_rels/workbook.xml.rels"))
            .expect("workbook rels should be readable");
        let workbook_rels_2 = fs::read_to_string(output_dir_2.join("xl/_rels/workbook.xml.rels"))
            .expect("workbook rels should be readable");
        assert!(
            !workbook_rels_1.contains("calcChain"),
            "workbook.xml.rels should not reference calcChain in output 1"
        );
        assert!(
            !workbook_rels_2.contains("calcChain"),
            "workbook.xml.rels should not reference calcChain in output 2"
        );

        let content_types_1 = fs::read_to_string(output_dir_1.join("[Content_Types].xml"))
            .expect("content types should be readable");
        let content_types_2 = fs::read_to_string(output_dir_2.join("[Content_Types].xml"))
            .expect("content types should be readable");
        assert!(
            !content_types_1.contains("/xl/calcChain.xml"),
            "[Content_Types].xml should not include calcChain override in output 1"
        );
        assert!(
            !content_types_2.contains("/xl/calcChain.xml"),
            "[Content_Types].xml should not include calcChain override in output 2"
        );
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
