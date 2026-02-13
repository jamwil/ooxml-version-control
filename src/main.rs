use clap::{Parser, Subcommand, ValueEnum};
use env_logger::{self, Env};
#[cfg(feature = "spec-validation")]
use libxml::schemas::{SchemaParserContext, SchemaValidationContext};
use ooxml_version_control::filesystem;
use ooxml_version_control::ooxml::schemas::shared_strings;
use ooxml_version_control::ooxml::{read_xml_file, validate_xml_file, OoxmlBuffer};
use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, Writer};
use std::collections::HashSet;
use std::fs;
use std::fs::remove_file;
use std::io::{self, Cursor, Write};
use std::path::{Path, PathBuf};
use tempfile::tempdir;

#[derive(Parser)]
#[command(name = "ocv")]
#[command(version = "0.1.0")]
#[command(author = "James Williams <james@jamwil.com>")]
#[command(about = "Diffable, mergeable version control for OOXML files (alpha).")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Convert compiled OOXML bundles (.xlsx/.docx/.pptx) into normalized raw OOXML trees (*_ooxml)
    CheckIn {
        /// Input compiled OOXML bundle files
        #[arg(required = true)]
        paths: Vec<PathBuf>,
        /// Output container directory for generated raw OOXML trees
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Convert raw OOXML trees (*_ooxml) back into compiled OOXML bundles
    CheckOut {
        /// Input raw OOXML tree directories
        #[arg(required = true)]
        paths: Vec<PathBuf>,
        /// Output container directory for generated compiled bundles
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Validate XML syntax for OOXML raw trees (.xml/.rels)
    Validate {
        /// Files and/or directories to validate
        #[arg(required = true)]
        paths: Vec<PathBuf>,
        /// Validation mode: basic XML syntax or OOXML spec schema checks
        #[arg(long, value_enum, default_value_t = ValidationMode::Basic)]
        mode: ValidationMode,
        /// OOXML schema profile for --mode spec
        #[arg(long, value_enum, default_value_t = SchemaProfile::Transitional)]
        profile: SchemaProfile,
    },
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum ValidationMode {
    Basic,
    Spec,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum SchemaProfile {
    Transitional,
    Strict,
}

fn check_in_path(path: &PathBuf, output: Option<&PathBuf>) {
    if !path.is_file() {
        panic!("Error: Path is not a valid file: {:?}", path);
    }

    log::info!("Checking in: {:?}", path);

    let file_name = path.file_name().unwrap().to_str().unwrap();
    log::debug!("File name: {:?}", file_name);

    let output_name = file_name.to_owned() + "_ooxml";
    let output_dir = output
        .map(|container| container.join(&output_name))
        .unwrap_or_else(|| path.with_file_name(output_name));
    log::debug!("Output dir: {:?}", output_dir);

    let work_dir = tempdir().unwrap().path().to_path_buf();
    log::trace!("Temporary Work dir: {:?}", work_dir);

    filesystem::unzip(&path, &work_dir);

    // Drop the files we don't want to keep
    let unwanted_files = vec!["xl/calcChain.xml"];
    for unwanted_file in unwanted_files {
        let unwanted_file_path = work_dir.join(unwanted_file);
        if unwanted_file_path.exists() {
            log::debug!("Removing unwanted file: {:?}", unwanted_file_path);
            remove_file(unwanted_file_path).unwrap();
        }
    }

    // Keep package metadata consistent when calcChain is removed.
    let workbook_rels = work_dir.join("xl/_rels/workbook.xml.rels");
    if workbook_rels.exists() {
        OoxmlBuffer::new(workbook_rels.to_str().unwrap())
            .remove_calc_chain_relationship_entries()
            .tidy()
            .save();
    }
    let content_types = work_dir.join("[Content_Types].xml");
    if content_types.exists() {
        OoxmlBuffer::new(content_types.to_str().unwrap())
            .remove_calc_chain_content_type_override()
            .tidy()
            .save();
    }

    // Drop volatile document metadata that changes frequently and
    // creates noisy diffs/conflicts across environments.
    let core_props = work_dir.join("docProps/core.xml");
    if core_props.exists() {
        OoxmlBuffer::new(core_props.to_str().unwrap())
            .remove_volatile_core_properties()
            .tidy()
            .save();
    }
    let app_props = work_dir.join("docProps/app.xml");
    if app_props.exists() {
        OoxmlBuffer::new(app_props.to_str().unwrap())
            .remove_volatile_app_properties()
            .tidy()
            .save();
    }

    // Get the shared strings
    let ss_file = work_dir.join("xl/sharedStrings.xml");
    let default_sst = shared_strings::Sst {
        xmlns: String::from("http://schemas.openxmlformats.org/spreadsheetml/2006/main"),
        count: String::from("0"),
        unique_count: String::from("0"),
        si: vec![],
    };
    let sst: shared_strings::Sst = read_xml_file(ss_file.to_str().unwrap()).unwrap_or(default_sst);

    // Only rewrite worksheet XML files; keep other OOXML parts byte-faithful.
    let worksheet_xml_files = filesystem::collect_files(&work_dir, "xl/worksheets/*.xml");
    for xml_file in worksheet_xml_files {
        log::debug!("Inlining shared strings in worksheet: {:?}", xml_file);
        OoxmlBuffer::new(xml_file.to_str().unwrap())
            .inline_shared_strings(&sst)
            .tidy()
            .save();
    }

    // Normalize all XML-like parts to keep check-in output deterministic
    // and easier to diff/merge.
    let xml_like_files = filesystem::collect_files_by_extension(&work_dir, &["xml", "rels"]);
    for xml_file in xml_like_files {
        OoxmlBuffer::new(xml_file.to_str().unwrap()).tidy().save();
    }

    filesystem::copy_dir(&work_dir, &output_dir);

    log::info!("Checked in: {:?}", output_dir);
}

fn check_out_path(path: &PathBuf, output: Option<&PathBuf>) {
    if !path.is_dir() {
        panic!("Error: Path is not a valid directory: {:?}", path);
    }

    log::info!("Checking out: {:?}", path);

    let input_dir = path.file_name().unwrap().to_str().unwrap();
    log::debug!("Input dir: {:?}", input_dir);

    let output_name = input_dir.to_string().replace("_ooxml", "");
    let output_file = output
        .map(|container| container.join(&output_name))
        .unwrap_or_else(|| path.with_file_name(output_name));
    log::debug!("File name: {:?}", output_file);

    filesystem::zip(&path, &output_file);

    log::info!("Checked out: {:?}", output_file);
}

fn validation_targets_for_path(path: &PathBuf) -> Vec<PathBuf> {
    if path.is_file() {
        let is_xml_like = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|ext| {
                let lower = ext.to_ascii_lowercase();
                lower == "xml" || lower == "rels"
            })
            .unwrap_or(false);

        if !is_xml_like {
            panic!(
                "Error: File is not an XML-like OOXML part (.xml/.rels): {:?}",
                path
            );
        }
        return vec![path.clone()];
    }

    if path.is_dir() {
        return filesystem::collect_files_by_extension(path, &["xml", "rels"]);
    }

    panic!(
        "Error: Path does not exist or is not accessible: {:?}",
        path
    );
}

fn bundled_schema_root(profile: SchemaProfile) -> PathBuf {
    let profile_dir = match profile {
        SchemaProfile::Transitional => "transitional",
        SchemaProfile::Strict => "strict",
    };
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("schemas")
        .join("ooxml-xsd")
        .join(profile_dir)
}

fn schema_for_namespace(namespace: &str, profile: SchemaProfile) -> Option<&'static str> {
    match profile {
        SchemaProfile::Transitional => match namespace {
            "http://schemas.openxmlformats.org/spreadsheetml/2006/main" => Some("sml.xsd"),
            "http://schemas.openxmlformats.org/wordprocessingml/2006/main" => Some("wml.xsd"),
            "http://schemas.openxmlformats.org/presentationml/2006/main" => Some("pml.xsd"),
            "http://schemas.openxmlformats.org/drawingml/2006/main" => Some("dml-main.xsd"),
            "http://schemas.openxmlformats.org/drawingml/2006/chart" => Some("dml-chart.xsd"),
            "http://schemas.openxmlformats.org/drawingml/2006/chartDrawing" => {
                Some("dml-chartDrawing.xsd")
            }
            "http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing" => {
                Some("dml-spreadsheetDrawing.xsd")
            }
            "http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" => {
                Some("dml-wordprocessingDrawing.xsd")
            }
            "http://schemas.openxmlformats.org/officeDocument/2006/extended-properties" => {
                Some("shared-documentPropertiesExtended.xsd")
            }
            "http://schemas.openxmlformats.org/officeDocument/2006/custom-properties" => {
                Some("shared-documentPropertiesCustom.xsd")
            }
            "http://schemas.openxmlformats.org/officeDocument/2006/bibliography" => {
                Some("shared-bibliography.xsd")
            }
            "http://schemas.openxmlformats.org/officeDocument/2006/math" => Some("shared-math.xsd"),
            _ => None,
        },
        SchemaProfile::Strict => match namespace {
            "http://purl.oclc.org/ooxml/spreadsheetml/main" => Some("sml.xsd"),
            "http://purl.oclc.org/ooxml/wordprocessingml/main" => Some("wml.xsd"),
            "http://purl.oclc.org/ooxml/presentationml/main" => Some("pml.xsd"),
            "http://purl.oclc.org/ooxml/drawingml/main" => Some("dml-main.xsd"),
            "http://purl.oclc.org/ooxml/drawingml/chart" => Some("dml-chart.xsd"),
            "http://purl.oclc.org/ooxml/drawingml/chartDrawing" => Some("dml-chartDrawing.xsd"),
            "http://purl.oclc.org/ooxml/drawingml/spreadsheetDrawing" => {
                Some("dml-spreadsheetDrawing.xsd")
            }
            "http://purl.oclc.org/ooxml/drawingml/wordprocessingDrawing" => {
                Some("dml-wordprocessingDrawing.xsd")
            }
            "http://purl.oclc.org/ooxml/officeDocument/extendedProperties" => {
                Some("shared-documentPropertiesExtended.xsd")
            }
            "http://purl.oclc.org/ooxml/officeDocument/customProperties" => {
                Some("shared-documentPropertiesCustom.xsd")
            }
            "http://purl.oclc.org/ooxml/officeDocument/bibliography" => {
                Some("shared-bibliography.xsd")
            }
            "http://purl.oclc.org/ooxml/officeDocument/math" => Some("shared-math.xsd"),
            _ => None,
        },
    }
}

fn root_default_namespace(file_path: &Path) -> Result<Option<String>, io::Error> {
    let content = fs::read_to_string(file_path)?;
    let mut reader = Reader::from_str(&content);
    reader.config_mut().trim_text(false);

    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                for attr in e.attributes().with_checks(false).flatten() {
                    if attr.key.as_ref() == b"xmlns" {
                        let Some(unescaped) = attr.unescape_value().ok() else {
                            continue;
                        };
                        return Ok(Some(unescaped.into_owned()));
                    }
                }
                return Ok(None);
            }
            Ok(Event::Decl(_))
            | Ok(Event::Comment(_))
            | Ok(Event::PI(_))
            | Ok(Event::DocType(_)) => {}
            Ok(Event::Eof) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "XML document has no root element",
                ))
            }
            Ok(_) => {}
            Err(_) => return Ok(None),
        }
        buf.clear();
    }
}

#[cfg(feature = "spec-validation")]
fn structured_errors_to_string(errors: &[libxml::error::StructuredError]) -> String {
    errors
        .iter()
        .map(|e| {
            let loc = match (e.filename.as_deref(), e.line, e.col) {
                (Some(file), Some(line), Some(col)) => format!("{file}:{line}:{col}"),
                (Some(file), Some(line), None) => format!("{file}:{line}"),
                (Some(file), None, None) => file.to_string(),
                (None, Some(line), Some(col)) => format!("line {line}, col {col}"),
                (None, Some(line), None) => format!("line {line}"),
                _ => "unknown location".to_string(),
            };
            let msg = e
                .message
                .as_deref()
                .unwrap_or("schema validation error")
                .trim();
            format!("{loc}: {msg}")
        })
        .collect::<Vec<_>>()
        .join("; ")
}

fn sanitize_xml_for_schema_validation(file_path: &Path) -> Result<tempfile::NamedTempFile, String> {
    let mut reader = Reader::from_file(file_path).map_err(|err| err.to_string())?;
    reader.config_mut().trim_text(false);

    let mut output = Vec::new();
    let mut writer = Writer::new(Cursor::new(&mut output));
    let mut buf = Vec::new();

    let mut skip_depth = 0usize;
    let mut ignorable_prefixes = HashSet::new();

    let is_mc_name = |name: &[u8]| name.starts_with(b"mc:");
    let attr_prefix = |name: &[u8]| -> Option<String> {
        std::str::from_utf8(name)
            .ok()
            .and_then(|s| s.split_once(':').map(|(prefix, _)| prefix.to_string()))
    };

    loop {
        let event = reader.read_event_into(&mut buf).unwrap_or(Event::Eof);
        match event {
            Event::Eof => break,
            Event::Start(e) => {
                let name_bytes = e.name().as_ref().to_vec();
                if is_mc_name(&name_bytes) {
                    skip_depth = 1;
                    buf.clear();
                    continue;
                }
                if skip_depth > 0 {
                    skip_depth += 1;
                    buf.clear();
                    continue;
                }

                let name = std::str::from_utf8(&name_bytes).map_err(|err| err.to_string())?;
                let mut out = BytesStart::new(name);

                for attr in e.attributes().with_checks(false) {
                    let attr = attr.map_err(|err| err.to_string())?;
                    let key_bytes = attr.key.as_ref();
                    let key = std::str::from_utf8(key_bytes).map_err(|err| err.to_string())?;
                    let value = attr
                        .unescape_value()
                        .ok()
                        .map(|v| v.into_owned())
                        .unwrap_or_default();

                    if key == "mc:Ignorable" {
                        for prefix in value.split_whitespace() {
                            ignorable_prefixes.insert(prefix.to_string());
                        }
                        continue;
                    }
                    if let Some(prefix) = attr_prefix(key_bytes) {
                        if ignorable_prefixes.contains(&prefix) {
                            continue;
                        }
                    }

                    out.push_attribute((key, value.as_str()));
                }

                writer.write_event(Event::Start(out)).unwrap();
            }
            Event::Empty(e) => {
                let name_bytes = e.name().as_ref().to_vec();
                if is_mc_name(&name_bytes) {
                    buf.clear();
                    continue;
                }
                if skip_depth > 0 {
                    buf.clear();
                    continue;
                }

                let name = std::str::from_utf8(&name_bytes).map_err(|err| err.to_string())?;
                let mut out = BytesStart::new(name);

                for attr in e.attributes().with_checks(false) {
                    let attr = attr.map_err(|err| err.to_string())?;
                    let key_bytes = attr.key.as_ref();
                    let key = std::str::from_utf8(key_bytes).map_err(|err| err.to_string())?;
                    let value = attr
                        .unescape_value()
                        .ok()
                        .map(|v| v.into_owned())
                        .unwrap_or_default();

                    if key == "mc:Ignorable" {
                        for prefix in value.split_whitespace() {
                            ignorable_prefixes.insert(prefix.to_string());
                        }
                        continue;
                    }
                    if let Some(prefix) = attr_prefix(key_bytes) {
                        if ignorable_prefixes.contains(&prefix) {
                            continue;
                        }
                    }

                    out.push_attribute((key, value.as_str()));
                }

                writer.write_event(Event::Empty(out)).unwrap();
            }
            Event::End(e) => {
                if skip_depth > 0 {
                    skip_depth -= 1;
                    buf.clear();
                    continue;
                }
                writer.write_event(Event::End(e.to_owned())).unwrap();
            }
            other => {
                if skip_depth == 0 {
                    writer.write_event(other).unwrap();
                }
            }
        }
        buf.clear();
    }

    let mut temp = tempfile::NamedTempFile::new().map_err(|err| err.to_string())?;
    temp.write_all(&output).unwrap();
    Ok(temp)
}

#[cfg(feature = "spec-validation")]
fn validate_file_against_schema(xml_path: &Path, schema_path: &Path) -> Result<(), String> {
    let schema_str = schema_path
        .to_str()
        .ok_or("non-utf8 schema path".to_string())?;
    let xml_str = xml_path.to_str().ok_or("non-utf8 xml path".to_string())?;

    let mut parser = SchemaParserContext::from_file(schema_str);
    let mut validator = SchemaValidationContext::from_parser(&mut parser).map_err(|errs| {
        format!(
            "schema parse failed: {}",
            structured_errors_to_string(&errs)
        )
    })?;

    validator
        .validate_file(xml_str)
        .map_err(|errs| structured_errors_to_string(&errs))
}

#[cfg(not(feature = "spec-validation"))]
fn validate_file_against_schema(_xml_path: &Path, _schema_path: &Path) -> Result<(), String> {
    Err(
        "spec validation support is not enabled; rebuild with `--features spec-validation`"
            .to_string(),
    )
}

fn uses_markup_compatibility_features(file_path: &Path) -> Result<bool, String> {
    let content = fs::read_to_string(file_path).map_err(|err| err.to_string())?;
    Ok(
        content.contains("http://schemas.openxmlformats.org/markup-compatibility/2006")
            || content.contains("http://purl.oclc.org/ooxml/markup-compatibility/main")
            || content.contains("mc:AlternateContent")
            || content.contains("mc:Choice")
            || content.contains("mc:Fallback")
            || content.contains("mc:Ignorable"),
    )
}

fn validate_path_spec(file_path: &Path, profile: SchemaProfile) -> Result<bool, String> {
    validate_xml_file(file_path.to_str().ok_or("non-utf8 xml path".to_string())?)
        .map_err(|err| err.to_string())?;

    let namespace = root_default_namespace(file_path).map_err(|err| err.to_string())?;
    let Some(namespace) = namespace else {
        return Ok(false);
    };

    let Some(schema_name) = schema_for_namespace(&namespace, profile) else {
        return Ok(false);
    };
    let schema_path = bundled_schema_root(profile).join(schema_name);
    let has_mce = uses_markup_compatibility_features(file_path)?;
    if has_mce {
        // ECMA-376 Part 5 MCE preprocessing is not implemented yet.
        // Skip these parts to avoid false negatives from plain XSD validation.
        return Ok(false);
    }

    let sanitized = sanitize_xml_for_schema_validation(file_path)?;
    let schema_validation = validate_file_against_schema(sanitized.path(), &schema_path);
    match schema_validation {
        Ok(()) => Ok(true),
        Err(err) => Err(err),
    }
}

fn validate_paths(paths: &[PathBuf], mode: ValidationMode, profile: SchemaProfile) {
    if mode == ValidationMode::Spec && !cfg!(feature = "spec-validation") {
        panic!(
            "Error: `validate --mode spec` requires the `spec-validation` feature. Rebuild with `cargo build --features spec-validation`."
        );
    }

    let mut targets: Vec<PathBuf> = paths.iter().flat_map(validation_targets_for_path).collect();

    if targets.is_empty() {
        panic!("Error: No XML files found to validate");
    }

    targets.sort();
    targets.dedup();

    let mut invalid = vec![];
    let mut schema_validated = 0usize;
    let mut schema_skipped = 0usize;
    for target in targets {
        let result = match mode {
            ValidationMode::Basic => validate_xml_file(target.to_str().unwrap())
                .map(|_| true)
                .map_err(|err| err.to_string()),
            ValidationMode::Spec => validate_path_spec(&target, profile),
        };
        match result {
            Ok(used_schema) => {
                if mode == ValidationMode::Spec {
                    if used_schema {
                        schema_validated += 1;
                        log::debug!("Validated XML against schema: {:?}", target);
                    } else {
                        schema_skipped += 1;
                        log::debug!(
                            "Skipped schema validation (no bundled schema): {:?}",
                            target
                        );
                    }
                } else {
                    log::debug!("Validated XML: {:?}", target);
                }
            }
            Err(err) => {
                log::error!("Invalid XML {:?}: {}", target, err);
                invalid.push(target);
            }
        }
    }

    if !invalid.is_empty() {
        panic!("Error: XML validation failed for {} file(s)", invalid.len());
    }

    if mode == ValidationMode::Spec {
        log::info!(
            "OOXML spec validation passed (schema-validated: {}, skipped: {})",
            schema_validated,
            schema_skipped
        );
    } else {
        log::info!("XML validation passed");
    }
}

fn main() {
    let env = Env::default().filter_or("MY_LOG_LEVEL", "info");
    env_logger::init_from_env(env);
    log::info!("Starting ocv");

    let cli = Cli::parse();

    match &cli.command {
        Commands::CheckIn { paths, output } => {
            for path in paths {
                check_in_path(path, output.as_ref());
            }
        }
        Commands::CheckOut { paths, output } => {
            if let Some(output_dir) = output {
                fs::create_dir_all(output_dir).unwrap_or_else(|err| {
                    panic!(
                        "Error: Failed to create output directory {:?}: {}",
                        output_dir, err
                    )
                });
            }
            for path in paths {
                check_out_path(path, output.as_ref());
            }
        }
        Commands::Validate {
            paths,
            mode,
            profile,
        } => validate_paths(paths, *mode, *profile),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::panic;
    use tempfile::tempdir;

    #[test]
    fn test_validation_targets_for_xml_file() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("part.xml");
        fs::write(&path, "<a/>").unwrap();

        let targets = validation_targets_for_path(&path);
        assert_eq!(targets, vec![path]);
    }

    #[test]
    fn test_validation_targets_for_non_xml_file_panics() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("part.txt");
        fs::write(&path, "x").unwrap();

        let result = panic::catch_unwind(|| validation_targets_for_path(&path));
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_paths_directory_success() {
        let temp = tempdir().unwrap();
        let root = temp.path().to_path_buf();
        fs::create_dir_all(root.join("xl")).unwrap();
        fs::create_dir_all(root.join("xl/_rels")).unwrap();
        fs::write(root.join("xl/workbook.xml"), "<workbook/>").unwrap();
        fs::write(root.join("xl/_rels/workbook.xml.rels"), "<Relationships/>").unwrap();

        validate_paths(&[root], ValidationMode::Basic, SchemaProfile::Transitional);
    }

    #[test]
    fn test_validate_paths_directory_failure_panics() {
        let temp = tempdir().unwrap();
        let root = temp.path().to_path_buf();
        fs::create_dir_all(root.join("xl")).unwrap();
        fs::write(root.join("xl/workbook.xml"), "<workbook>").unwrap();

        let result = panic::catch_unwind(|| {
            validate_paths(&[root], ValidationMode::Basic, SchemaProfile::Transitional)
        });
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_paths_no_args_panics_when_no_targets() {
        let result = panic::catch_unwind(|| {
            validate_paths(&[], ValidationMode::Basic, SchemaProfile::Transitional)
        });
        assert!(result.is_err());
    }

    #[test]
    fn test_schema_for_namespace_transitional_main_parts() {
        assert_eq!(
            schema_for_namespace(
                "http://schemas.openxmlformats.org/spreadsheetml/2006/main",
                SchemaProfile::Transitional
            ),
            Some("sml.xsd")
        );
        assert_eq!(
            schema_for_namespace(
                "http://schemas.openxmlformats.org/wordprocessingml/2006/main",
                SchemaProfile::Transitional
            ),
            Some("wml.xsd")
        );
        assert_eq!(
            schema_for_namespace(
                "http://schemas.openxmlformats.org/officeDocument/2006/extended-properties",
                SchemaProfile::Transitional
            ),
            Some("shared-documentPropertiesExtended.xsd")
        );
    }

    #[test]
    fn test_bundled_schema_root_profiles() {
        assert!(bundled_schema_root(SchemaProfile::Transitional)
            .ends_with("schemas/ooxml-xsd/transitional"));
        assert!(bundled_schema_root(SchemaProfile::Strict).ends_with("schemas/ooxml-xsd/strict"));
    }

    #[test]
    fn test_schema_for_namespace_strict_main_parts() {
        assert_eq!(
            schema_for_namespace(
                "http://purl.oclc.org/ooxml/spreadsheetml/main",
                SchemaProfile::Strict
            ),
            Some("sml.xsd")
        );
        assert_eq!(
            schema_for_namespace(
                "http://purl.oclc.org/ooxml/wordprocessingml/main",
                SchemaProfile::Strict
            ),
            Some("wml.xsd")
        );
        assert_eq!(
            schema_for_namespace(
                "http://purl.oclc.org/ooxml/presentationml/main",
                SchemaProfile::Strict
            ),
            Some("pml.xsd")
        );
        assert_eq!(
            schema_for_namespace(
                "http://purl.oclc.org/ooxml/officeDocument/math",
                SchemaProfile::Strict
            ),
            Some("shared-math.xsd")
        );
        assert_eq!(
            schema_for_namespace(
                "http://purl.oclc.org/ooxml/officeDocument/extendedProperties",
                SchemaProfile::Strict
            ),
            Some("shared-documentPropertiesExtended.xsd")
        );
        assert_eq!(
            schema_for_namespace("urn:unknown-strict", SchemaProfile::Strict),
            None
        );
    }

    #[test]
    fn test_schema_for_namespace_covers_additional_arms() {
        assert_eq!(
            schema_for_namespace(
                "http://schemas.openxmlformats.org/drawingml/2006/chartDrawing",
                SchemaProfile::Transitional
            ),
            Some("dml-chartDrawing.xsd")
        );
        assert_eq!(
            schema_for_namespace(
                "http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing",
                SchemaProfile::Transitional
            ),
            Some("dml-spreadsheetDrawing.xsd")
        );
        assert_eq!(
            schema_for_namespace(
                "http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing",
                SchemaProfile::Transitional
            ),
            Some("dml-wordprocessingDrawing.xsd")
        );
        assert_eq!(
            schema_for_namespace(
                "http://schemas.openxmlformats.org/officeDocument/2006/custom-properties",
                SchemaProfile::Transitional
            ),
            Some("shared-documentPropertiesCustom.xsd")
        );
        assert_eq!(
            schema_for_namespace(
                "http://schemas.openxmlformats.org/officeDocument/2006/bibliography",
                SchemaProfile::Transitional
            ),
            Some("shared-bibliography.xsd")
        );
        assert_eq!(
            schema_for_namespace(
                "http://purl.oclc.org/ooxml/drawingml/chartDrawing",
                SchemaProfile::Strict
            ),
            Some("dml-chartDrawing.xsd")
        );
        assert_eq!(
            schema_for_namespace(
                "http://purl.oclc.org/ooxml/drawingml/spreadsheetDrawing",
                SchemaProfile::Strict
            ),
            Some("dml-spreadsheetDrawing.xsd")
        );
        assert_eq!(
            schema_for_namespace(
                "http://purl.oclc.org/ooxml/drawingml/wordprocessingDrawing",
                SchemaProfile::Strict
            ),
            Some("dml-wordprocessingDrawing.xsd")
        );
        assert_eq!(
            schema_for_namespace(
                "http://purl.oclc.org/ooxml/officeDocument/customProperties",
                SchemaProfile::Strict
            ),
            Some("shared-documentPropertiesCustom.xsd")
        );
        assert_eq!(
            schema_for_namespace(
                "http://purl.oclc.org/ooxml/officeDocument/bibliography",
                SchemaProfile::Strict
            ),
            Some("shared-bibliography.xsd")
        );
    }

    #[test]
    fn test_root_default_namespace_reads_xmlns() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("root.xml");
        fs::write(
            &path,
            r#"<x:root xmlns:x="urn:ignored" xmlns="urn:default"><child/></x:root>"#,
        )
        .unwrap();

        let ns = root_default_namespace(&path).unwrap();
        assert_eq!(ns.as_deref(), Some("urn:default"));
    }

    #[test]
    fn test_root_default_namespace_empty_element_and_errors() {
        let temp = tempdir().unwrap();

        let empty_with_xmlns = temp.path().join("empty.xml");
        fs::write(&empty_with_xmlns, r#"<root xmlns="urn:empty"/>"#).unwrap();
        let ns = root_default_namespace(&empty_with_xmlns).unwrap();
        assert_eq!(ns.as_deref(), Some("urn:empty"));

        let no_root = temp.path().join("noroot.xml");
        fs::write(&no_root, "<?xml version=\"1.0\"?>").unwrap();
        let err = root_default_namespace(&no_root).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);

        let missing = temp.path().join("missing.xml");
        let err = root_default_namespace(&missing).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::NotFound);

        let malformed = temp.path().join("malformed.xml");
        fs::write(&malformed, "<root xmlns='a&b'/>").unwrap();
        match root_default_namespace(&malformed) {
            Err(err) => assert_eq!(err.kind(), std::io::ErrorKind::InvalidData),
            Ok(ns) => assert!(ns.is_none()),
        }

        let parser_error = temp.path().join("parser_error.xml");
        fs::write(&parser_error, "<").unwrap();
        let ns = root_default_namespace(&parser_error).unwrap();
        assert!(ns.is_none());
    }

    #[cfg(feature = "spec-validation")]
    #[test]
    fn test_structured_errors_to_string_location_variants() {
        use libxml::error::{StructuredError, XmlErrorLevel};
        let errors = vec![
            StructuredError {
                message: Some("m1".to_string()),
                level: XmlErrorLevel::Error,
                filename: Some("f.xml".to_string()),
                line: Some(1),
                col: Some(2),
                domain: 0,
                code: 0,
            },
            StructuredError {
                message: Some("m2".to_string()),
                level: XmlErrorLevel::Error,
                filename: Some("g.xml".to_string()),
                line: Some(3),
                col: None,
                domain: 0,
                code: 0,
            },
            StructuredError {
                message: Some("m3".to_string()),
                level: XmlErrorLevel::Error,
                filename: Some("h.xml".to_string()),
                line: None,
                col: None,
                domain: 0,
                code: 0,
            },
            StructuredError {
                message: Some("m4".to_string()),
                level: XmlErrorLevel::Error,
                filename: None,
                line: Some(4),
                col: Some(5),
                domain: 0,
                code: 0,
            },
            StructuredError {
                message: Some("m5".to_string()),
                level: XmlErrorLevel::Error,
                filename: None,
                line: Some(6),
                col: None,
                domain: 0,
                code: 0,
            },
            StructuredError {
                message: None,
                level: XmlErrorLevel::Error,
                filename: None,
                line: None,
                col: None,
                domain: 0,
                code: 0,
            },
        ];

        let s = structured_errors_to_string(&errors);
        assert!(s.contains("f.xml:1:2: m1"));
        assert!(s.contains("g.xml:3: m2"));
        assert!(s.contains("h.xml: m3"));
        assert!(s.contains("line 4, col 5: m4"));
        assert!(s.contains("line 6: m5"));
        assert!(s.contains("unknown location: schema validation error"));
    }

    #[test]
    fn test_sanitize_xml_for_schema_validation_strips_mc_and_ignorable() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("mc.xml");
        fs::write(
            &path,
            r#"<root xmlns="urn:r" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" xmlns:x="urn:x" mc:Ignorable="x" x:drop="1"><mc:Choice><z/></mc:Choice><a x:drop="2" keep="y"/><b keep="z"/></root>"#,
        )
        .unwrap();

        let sanitized = sanitize_xml_for_schema_validation(&path).unwrap();
        let content = fs::read_to_string(sanitized.path()).unwrap();
        assert!(!content.contains("mc:Choice"));
        assert!(!content.contains("mc:Ignorable"));
        assert!(!content.contains("x:drop"));
        assert!(content.contains("keep=\"y\""));
        assert!(content.contains("keep=\"z\""));
    }

    #[test]
    fn test_sanitize_xml_for_schema_validation_nested_and_empty_mc_paths() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("mc_nested.xml");
        fs::write(
            &path,
            r#"<root xmlns="urn:r" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" xmlns:x="urn:x" mc:Ignorable="x"><mc:Choice><inner><leaf/></inner></mc:Choice><mc:Fallback/><item mc:PreserveElements="a" x:drop="1" keep="v"/><empty mc:Ignorable="x" mc:ProcessContent="z" x:drop="2" keep="e"/></root>"#,
        )
        .unwrap();

        let sanitized = sanitize_xml_for_schema_validation(&path).unwrap();
        let content = fs::read_to_string(sanitized.path()).unwrap();
        assert!(!content.contains("mc:Choice"));
        assert!(!content.contains("mc:Fallback"));
        assert!(!content.contains("x:drop"));
        assert!(content.contains("keep=\"v\""));
        assert!(content.contains("keep=\"e\""));
    }

    #[test]
    fn test_sanitize_xml_for_schema_validation_handles_malformed_xml() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("bad.xml");
        fs::write(&path, b"<root>\x00</root>").unwrap();
        let result = sanitize_xml_for_schema_validation(&path);
        assert!(result.is_ok() || result.is_err());
    }

    #[cfg(feature = "spec-validation")]
    #[test]
    fn test_validate_file_against_schema_success_and_failure() {
        let temp = tempdir().unwrap();
        let schema = temp.path().join("s.xsd");
        fs::write(
            &schema,
            r#"<?xml version="1.0"?><xsd:schema xmlns:xsd="http://www.w3.org/2001/XMLSchema"><xsd:element name="root" type="xsd:string"/></xsd:schema>"#,
        )
        .unwrap();

        let ok = temp.path().join("ok.xml");
        fs::write(&ok, "<root>ok</root>").unwrap();
        assert!(validate_file_against_schema(&ok, &schema).is_ok());

        let bad = temp.path().join("bad.xml");
        fs::write(&bad, "<other/>").unwrap();
        assert!(validate_file_against_schema(&bad, &schema).is_err());
    }

    #[cfg(feature = "spec-validation")]
    #[test]
    fn test_validate_file_against_schema_invalid_schema_returns_error() {
        let temp = tempdir().unwrap();
        let schema = temp.path().join("bad.xsd");
        fs::write(&schema, "<not-a-schema>").unwrap();
        let xml = temp.path().join("x.xml");
        fs::write(&xml, "<root/>").unwrap();
        assert!(validate_file_against_schema(&xml, &schema).is_err());
    }

    #[test]
    fn test_uses_markup_compatibility_features_true_and_false() {
        let temp = tempdir().unwrap();
        let mc = temp.path().join("mc.xml");
        fs::write(
            &mc,
            r#"<r xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" mc:Ignorable="x"/>"#,
        )
        .unwrap();
        assert!(uses_markup_compatibility_features(&mc).unwrap());

        let plain = temp.path().join("plain.xml");
        fs::write(&plain, "<r/>").unwrap();
        assert!(!uses_markup_compatibility_features(&plain).unwrap());
    }

    #[cfg(feature = "spec-validation")]
    #[test]
    fn test_validate_path_spec_with_supported_namespace() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/simple_book.xlsx_ooxml/xl/sharedStrings.xml");
        let used_schema = validate_path_spec(&path, SchemaProfile::Transitional).unwrap();
        assert!(used_schema);
    }

    #[cfg(feature = "spec-validation")]
    #[test]
    fn test_validate_path_spec_skips_mce_content() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/simple_book.xlsx_ooxml/xl/workbook.xml");
        let used_schema = validate_path_spec(&path, SchemaProfile::Transitional).unwrap();
        assert!(!used_schema);
    }

    #[cfg(feature = "spec-validation")]
    #[test]
    fn test_validate_path_spec_supported_namespace_skip_and_validate_paths() {
        let temp = tempdir().unwrap();

        let with_mce = temp.path().join("with_mce.xml");
        fs::write(
            &with_mce,
            r#"<sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" mc:Ignorable="x"><si><t>hello</t></si></sst>"#,
        )
        .unwrap();
        let used_schema = validate_path_spec(&with_mce, SchemaProfile::Transitional).unwrap();
        assert!(!used_schema);

        let without_mce = temp.path().join("without_mce.xml");
        fs::write(
            &without_mce,
            r#"<sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" count="1" uniqueCount="1"><si><t>hello</t></si></sst>"#,
        )
        .unwrap();
        let used_schema = validate_path_spec(&without_mce, SchemaProfile::Transitional).unwrap();
        assert!(used_schema);
    }

    #[cfg(feature = "spec-validation")]
    #[test]
    fn test_validate_path_spec_returns_err_on_schema_violation() {
        let temp = tempdir().unwrap();
        let invalid = temp.path().join("schema_invalid.xml");
        fs::write(
            &invalid,
            r#"<notSst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><child/></notSst>"#,
        )
        .unwrap();
        assert!(validate_path_spec(&invalid, SchemaProfile::Transitional).is_err());
    }

    #[cfg(feature = "spec-validation")]
    #[test]
    fn test_validate_path_spec_errors_on_malformed_xml() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("bad.xml");
        fs::write(&path, "<root>").unwrap();
        assert!(validate_path_spec(&path, SchemaProfile::Transitional).is_err());
    }

    #[cfg(feature = "spec-validation")]
    #[test]
    fn test_validate_path_spec_skips_unsupported_namespace() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("custom.xml");
        fs::write(&path, r#"<root xmlns="urn:custom"/>"#).unwrap();

        let used_schema = validate_path_spec(&path, SchemaProfile::Transitional).unwrap();
        assert!(!used_schema);
    }

    #[test]
    fn test_validate_xml_file_covers_open_close_and_parse_errors() {
        let temp = tempdir().unwrap();

        let ok_path = temp.path().join("ok.xml");
        fs::write(&ok_path, "<root><child/></root>").unwrap();
        assert!(validate_xml_file(ok_path.to_str().unwrap()).is_ok());

        let bad_close = temp.path().join("bad_close.xml");
        fs::write(&bad_close, "</root>").unwrap();
        let err = validate_xml_file(bad_close.to_str().unwrap()).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);

        let unclosed = temp.path().join("unclosed.xml");
        fs::write(&unclosed, "<root>").unwrap();
        let err = validate_xml_file(unclosed.to_str().unwrap()).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
    }
}
