use glob::glob;
use std::fs::{self, File};
use std::io;
use std::path::PathBuf;
use walkdir::WalkDir;
use zip::write::SimpleFileOptions;
use zip::{ZipArchive, ZipWriter};

/// Unzips a zip file (ooxml bundle) to a directory
pub fn unzip(input_file: &PathBuf, output_dir: &PathBuf) -> () {
    let file = File::open(input_file).unwrap();
    let mut archive = ZipArchive::new(file).unwrap();
    archive.extract(output_dir).unwrap();
    log::debug!("Unzipped {:?} to {:?}", input_file, output_dir);
}

/// Zips a directory to a zip file (ooxml bundle)
pub fn zip(input_dir: &PathBuf, output_file: &PathBuf) -> () {
    let file = File::create(output_file).unwrap();
    let mut writer = ZipWriter::new(file);

    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

    for entry in WalkDir::new(input_dir) {
        let entry = entry.unwrap();
        let path = entry.path();
        let name = path.strip_prefix(input_dir).unwrap();

        if path.is_file() {
            log::trace!("Adding file to zip: {:?}", path);
            writer.start_file(name.to_str().unwrap(), options).unwrap();
            let mut file = File::open(path).unwrap();
            io::copy(&mut file, &mut writer).unwrap();
            log::trace!("Added file to zip: {:?}", path);
        }
    }

    writer.finish().unwrap();
    log::debug!("Zipped {:?} to {:?}", input_dir, output_file);
}

/// Recursively copies a directory
pub fn copy_dir(input_dir: &PathBuf, output_dir: &PathBuf) -> () {
    if !output_dir.exists() {
        fs::create_dir_all(&output_dir).unwrap();
        log::debug!("Created directory: {:?}", output_dir);
    }

    for entry in WalkDir::new(&input_dir) {
        let entry = entry.unwrap();
        let path = entry.path();
        let name = path.strip_prefix(&input_dir).unwrap();

        if path.is_file() {
            log::trace!("Copying file: {:?}", path);
            let output_path = output_dir.join(name);
            if !output_path.parent().unwrap().exists() {
                fs::create_dir_all(output_path.parent().unwrap()).unwrap();
            }
            fs::copy(&path, &output_path).unwrap();
            log::trace!("Copied file: {:?}", output_path);
        }
    }

    log::debug!("Copied directory {:?} to {:?}", input_dir, output_dir);
}

/// Collect all files in a glob pattern
pub fn collect_files(dir: &PathBuf, pattern: &str) -> Vec<PathBuf> {
    let pattern = dir.join(pattern);
    let pattern = pattern.to_str().unwrap();
    glob(pattern).unwrap().filter_map(Result::ok).collect()
}

/// Recursively collect files by extension (without leading dots).
pub fn collect_files_by_extension(dir: &PathBuf, extensions: &[&str]) -> Vec<PathBuf> {
    WalkDir::new(dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_file())
        .filter(|entry| {
            let ext = entry.path().extension().and_then(|e| e.to_str());
            ext.map(|e| {
                let e = e.to_ascii_lowercase();
                extensions
                    .iter()
                    .any(|target| e == target.to_ascii_lowercase())
            })
            .unwrap_or(false)
        })
        .map(|entry| entry.path().to_path_buf())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[test]
    fn test_unzip() {
        let fixture = PathBuf::from("tests/fixtures/simple_book.xlsx");
        let output_dir = tempdir().unwrap().path().to_path_buf();

        unzip(&fixture, &output_dir);

        let expected_files = vec![
            "docProps/core.xml",
            "xl/workbook.xml",
            "xl/worksheets/sheet1.xml",
        ];

        for expected_file in &expected_files {
            let path = output_dir.join(expected_file);
            assert_eq!(path.is_file(), true);
        }
    }

    #[test]
    fn test_zip() {
        let input_dir = PathBuf::from("tests/fixtures/simple_book.xlsx_ooxml");
        let output_dir = tempdir().unwrap();
        let output_file = output_dir.path().join("simple_book.xlsx");

        zip(&input_dir, &output_file);

        assert_eq!(output_file.is_file(), true);

        let file = std::fs::File::open(&output_file).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();

        let expected_files = vec![
            "docProps/core.xml",
            "xl/workbook.xml",
            "xl/worksheets/sheet1.xml",
        ];

        for expected_file in &expected_files {
            let result = archive.by_name(expected_file);
            assert!(result.is_ok());
        }
    }

    #[test]
    fn test_copy_dir() {
        let input_dir = PathBuf::from("tests/fixtures/simple_book.xlsx_ooxml");
        let output_dir = tempdir().unwrap().path().to_path_buf();

        copy_dir(&input_dir, &output_dir);

        let expected_files = vec![
            "docProps/core.xml",
            "xl/workbook.xml",
            "xl/worksheets/sheet1.xml",
        ];

        for expected_file in &expected_files {
            let path = output_dir.join(expected_file);
            assert_eq!(path.is_file(), true);
        }
    }

    #[test]
    fn test_collect_files_by_extension() {
        let input_dir = PathBuf::from("tests/fixtures/simple_book.xlsx_ooxml");
        let files = collect_files_by_extension(&input_dir, &["xml", "rels"]);

        assert!(files.iter().any(|p| p.ends_with("docProps/core.xml")));
        assert!(files
            .iter()
            .any(|p| p.ends_with("xl/_rels/workbook.xml.rels")));
        assert!(!files.is_empty());
    }
}
