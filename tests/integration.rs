use assert_cmd::Command;
use ooxml_version_control::filesystem;
use ooxml_version_control::ooxml::{read_xml_file, schemas::shared_strings};
use std::fs;
use std::path::PathBuf;
use std::process::Command as ProcessCommand;
use tempfile::tempdir;

#[test]
fn test_ocv_binary_runs_help() {
    let mut cmd = Command::cargo_bin("ocv").unwrap();
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicates::str::contains("Diffable, mergeable version control for OOXML files"));
}

#[test]
fn test_check_in_with_valid_file() {
    let fixture = PathBuf::from("tests/fixtures/simple_book.xlsx");
    let mut cmd = Command::cargo_bin("ocv").unwrap();
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
    let mut cmd = Command::cargo_bin("ocv").unwrap();

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
    let mut cmd = Command::cargo_bin("ocv").unwrap();
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
    let mut cmd = Command::cargo_bin("ocv").unwrap();

    let invalid_folder = PathBuf::from("tests/fixtures/non_existent_folder.xlsx_ooxml");

    cmd.arg("check-out")
        .arg(invalid_folder)
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "Error: Path is not a valid directory",
        ));
}

#[test]
fn test_check_in_with_output_container_path() {
    let fixture = PathBuf::from("tests/fixtures/simple_book.xlsx");
    let mut cmd = Command::cargo_bin("ocv").unwrap();

    let temp_dir = tempdir().unwrap();
    let test_file_path = temp_dir.path().join("book.xlsx");
    let output_container = temp_dir.path().join("custom_output_container");
    fs::copy(&fixture, &test_file_path).unwrap();

    cmd.arg("check-in")
        .arg("-o")
        .arg(&output_container)
        .arg(&test_file_path)
        .assert()
        .success();

    let expected_raw_dir = output_container.join("book.xlsx_ooxml");
    assert!(expected_raw_dir.is_dir());
    assert!(expected_raw_dir.join("xl/workbook.xml").is_file());
}

#[test]
fn test_check_out_with_explicit_output_path() {
    let fixture = PathBuf::from("tests/fixtures/simple_book.xlsx_ooxml");
    let mut cmd = Command::cargo_bin("ocv").unwrap();

    let temp_dir = tempdir().unwrap();
    let test_folder_path = temp_dir.path().join("simple_book.xlsx_ooxml");
    filesystem::copy_dir(&fixture, &test_folder_path);
    let explicit_output = temp_dir.path().join("custom_name.xlsx");

    cmd.arg("check-out")
        .arg("-o")
        .arg(&explicit_output)
        .arg(&test_folder_path)
        .assert()
        .success();

    assert!(explicit_output.is_file());
}

#[test]
fn test_check_in_with_output_container_and_multiple_inputs_succeeds() {
    let fixture = PathBuf::from("tests/fixtures/simple_book.xlsx");
    let mut cmd = Command::cargo_bin("ocv").unwrap();

    let temp_dir = tempdir().unwrap();
    let file1 = temp_dir.path().join("book1.xlsx");
    let file2 = temp_dir.path().join("book2.xlsx");
    let output_container = temp_dir.path().join("raw");
    fs::copy(&fixture, &file1).unwrap();
    fs::copy(&fixture, &file2).unwrap();

    cmd.arg("check-in")
        .arg("-o")
        .arg(&output_container)
        .arg(&file1)
        .arg(&file2)
        .assert()
        .success();

    assert!(output_container.join("book1.xlsx_ooxml").is_dir());
    assert!(output_container.join("book2.xlsx_ooxml").is_dir());
}

#[test]
fn test_check_out_with_output_and_multiple_inputs_fails() {
    let fixture = PathBuf::from("tests/fixtures/simple_book.xlsx_ooxml");
    let mut cmd = Command::cargo_bin("ocv").unwrap();

    let temp_dir = tempdir().unwrap();
    let dir1 = temp_dir.path().join("book1.xlsx_ooxml");
    let dir2 = temp_dir.path().join("book2.xlsx_ooxml");
    filesystem::copy_dir(&fixture, &dir1);
    filesystem::copy_dir(&fixture, &dir2);

    cmd.arg("check-out")
        .arg("-o")
        .arg(temp_dir.path().join("single-output.xlsx"))
        .arg(&dir1)
        .arg(&dir2)
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "Error: --output/-o requires exactly one input path",
        ));
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
fn test_git_install_creates_hooks() {
    let temp_dir = tempdir().unwrap();
    let repo = temp_dir.path().to_path_buf();

    let init_status = ProcessCommand::new("git")
        .arg("init")
        .arg(&repo)
        .status()
        .unwrap();
    assert!(init_status.success());

    let mut cmd = Command::cargo_bin("ocv").unwrap();
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
    assert!(pre_commit.contains("run check-in with explicit OOXML bundle paths"));
    assert!(post_checkout.contains("run check-out with explicit *_ooxml paths"));
    assert!(post_merge.contains("run check-out with explicit *_ooxml paths"));
}

#[test]
fn test_check_in_out_and_validate_spec_with_docx_bundle() {
    let temp_dir = tempdir().unwrap();
    let raw_dir = temp_dir.path().join("sample.docx_ooxml");
    fs::create_dir_all(raw_dir.join("_rels")).unwrap();
    fs::create_dir_all(raw_dir.join("word")).unwrap();

    fs::write(
        raw_dir.join("[Content_Types].xml"),
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>"#,
    )
    .unwrap();
    fs::write(
        raw_dir.join("_rels/.rels"),
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#,
    )
    .unwrap();
    fs::write(
        raw_dir.join("word/document.xml"),
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p><w:r><w:t>Hello</w:t></w:r></w:p>
    <w:sectPr>
      <w:pgSz w:w="12240" w:h="15840"/>
      <w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="720" w:footer="720" w:gutter="0"/>
      <w:cols w:space="720"/>
      <w:docGrid w:linePitch="360"/>
    </w:sectPr>
  </w:body>
</w:document>"#,
    )
    .unwrap();

    let compiled_docx = temp_dir.path().join("sample.docx");
    filesystem::zip(&raw_dir, &compiled_docx);

    let mut check_in_cmd = Command::cargo_bin("ocv").unwrap();
    check_in_cmd
        .arg("check-in")
        .arg(&compiled_docx)
        .assert()
        .success();

    let checked_in_dir = temp_dir.path().join("sample.docx_ooxml");
    assert!(checked_in_dir.is_dir());
    assert!(checked_in_dir.join("word/document.xml").is_file());

    let mut validate_cmd = Command::cargo_bin("ocv").unwrap();
    validate_cmd
        .arg("validate")
        .arg("--mode")
        .arg("spec")
        .arg("--profile")
        .arg("transitional")
        .arg(&checked_in_dir)
        .assert()
        .success()
        .stderr(predicates::str::contains("OOXML spec validation passed"));

    let rebuilt_docx = temp_dir.path().join("sample.docx");
    if rebuilt_docx.exists() {
        fs::remove_file(&rebuilt_docx).unwrap();
    }
    let mut check_out_cmd = Command::cargo_bin("ocv").unwrap();
    check_out_cmd
        .arg("check-out")
        .arg(&checked_in_dir)
        .assert()
        .success();

    assert!(rebuilt_docx.is_file());
}

#[test]
fn test_macro_binary_parts_are_passed_through_for_xlsm() {
    let temp_dir = tempdir().unwrap();
    let raw_dir = temp_dir.path().join("macro_book.xlsm_ooxml");
    fs::create_dir_all(raw_dir.join("_rels")).unwrap();
    fs::create_dir_all(raw_dir.join("xl/_rels")).unwrap();
    fs::create_dir_all(raw_dir.join("xl/worksheets")).unwrap();

    let vba_payload: Vec<u8> = vec![0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB2, 0x00, 0x42, 0x99, 0xFE];

    fs::write(
        raw_dir.join("[Content_Types].xml"),
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Default Extension="bin" ContentType="application/vnd.ms-office.vbaProject"/>
  <Override PartName="/xl/workbook.xml" ContentType="application/vnd.ms-excel.sheet.macroEnabled.main+xml"/>
  <Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
</Types>"#,
    )
    .unwrap();
    fs::write(
        raw_dir.join("_rels/.rels"),
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>
</Relationships>"#,
    )
    .unwrap();
    fs::write(
        raw_dir.join("xl/workbook.xml"),
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"
          xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheets>
    <sheet name="Sheet1" sheetId="1" r:id="rId1"/>
  </sheets>
</workbook>"#,
    )
    .unwrap();
    fs::write(
        raw_dir.join("xl/_rels/workbook.xml.rels"),
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>
  <Relationship Id="rIdMacro" Type="http://schemas.microsoft.com/office/2006/relationships/vbaProject" Target="vbaProject.bin"/>
</Relationships>"#,
    )
    .unwrap();
    fs::write(
        raw_dir.join("xl/worksheets/sheet1.xml"),
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData>
    <row r="1"><c r="A1" t="str"><v>macro</v></c></row>
  </sheetData>
</worksheet>"#,
    )
    .unwrap();
    fs::write(raw_dir.join("xl/vbaProject.bin"), &vba_payload).unwrap();

    let compiled_xlsm = temp_dir.path().join("macro_book.xlsm");
    filesystem::zip(&raw_dir, &compiled_xlsm);

    let mut check_in_cmd = Command::cargo_bin("ocv").unwrap();
    check_in_cmd
        .arg("check-in")
        .arg(&compiled_xlsm)
        .assert()
        .success();

    let checked_in_dir = temp_dir.path().join("macro_book.xlsm_ooxml");
    assert!(checked_in_dir.is_dir());
    let checked_in_vba = fs::read(checked_in_dir.join("xl/vbaProject.bin")).unwrap();
    assert_eq!(checked_in_vba, vba_payload);

    let mut validate_cmd = Command::cargo_bin("ocv").unwrap();
    validate_cmd
        .arg("validate")
        .arg(&checked_in_dir)
        .assert()
        .success()
        .stderr(predicates::str::contains("XML validation passed"));

    if compiled_xlsm.exists() {
        fs::remove_file(&compiled_xlsm).unwrap();
    }
    let mut check_out_cmd = Command::cargo_bin("ocv").unwrap();
    check_out_cmd
        .arg("check-out")
        .arg(&checked_in_dir)
        .assert()
        .success();

    let rebuilt_dir = temp_dir.path().join("rebuilt_macro.xlsm_ooxml");
    filesystem::unzip(&compiled_xlsm, &rebuilt_dir);
    let rebuilt_vba = fs::read(rebuilt_dir.join("xl/vbaProject.bin")).unwrap();
    assert_eq!(rebuilt_vba, vba_payload);
}

#[test]
fn test_validate_with_valid_dir() {
    let fixture = PathBuf::from("tests/fixtures/simple_book.xlsx_ooxml");
    let mut cmd = Command::cargo_bin("ocv").unwrap();
    let test_folder_path: PathBuf;

    {
        let temp_dir = tempdir().unwrap();
        test_folder_path = temp_dir.path().join("simple_book.xlsx_ooxml");
        filesystem::copy_dir(&fixture, &test_folder_path);

        cmd.arg("validate")
            .arg(&test_folder_path)
            .assert()
            .success()
            .stderr(predicates::str::contains("XML validation passed"));
    }
}

#[test]
fn test_validate_with_invalid_xml_file() {
    let mut cmd = Command::cargo_bin("ocv").unwrap();

    let temp_dir = tempdir().unwrap();
    let invalid_file = temp_dir.path().join("broken.xml");
    fs::write(&invalid_file, "<worksheet>").unwrap();

    cmd.arg("validate")
        .arg(&invalid_file)
        .assert()
        .failure()
        .stderr(predicates::str::contains("XML validation failed"));
}

#[test]
fn test_validate_spec_with_supported_part() {
    let mut cmd = Command::cargo_bin("ocv").unwrap();
    let part = PathBuf::from("tests/fixtures/simple_book.xlsx_ooxml/xl/sharedStrings.xml");

    cmd.arg("validate")
        .arg("--mode")
        .arg("spec")
        .arg("--profile")
        .arg("transitional")
        .arg(&part)
        .assert()
        .success()
        .stderr(predicates::str::contains("OOXML spec validation passed"));
}

#[test]
fn test_validate_spec_with_fixture_dir() {
    let mut cmd = Command::cargo_bin("ocv").unwrap();
    let dir = PathBuf::from("tests/fixtures/simple_book.xlsx_ooxml");

    cmd.arg("validate")
        .arg("--mode")
        .arg("spec")
        .arg(&dir)
        .assert()
        .success()
        .stderr(predicates::str::contains("schema-validated"));
}
