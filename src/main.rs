use clap::{Parser, Subcommand};
use env_logger::{self, Env};
use ooxml_version_control::filesystem;
use ooxml_version_control::ooxml::schemas::shared_strings;
use ooxml_version_control::ooxml::{read_xml_file, OoxmlBuffer};
use std::fs::remove_file;
use std::path::PathBuf;
use tempfile::tempdir;

#[derive(Parser)]
#[command(name = "ooxml-version-control")]
#[command(version = "0.1.0")]
#[command(author = "James Williams <james@jamwil.com>")]
#[command(about = "Diffable, mergeable version control for OOXML files.")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Check in files or directories
    CheckIn {
        /// Files or directories to check in
        paths: Vec<PathBuf>,
    },
    /// Check out files or directories
    CheckOut {
        /// Files or directories to check out
        paths: Vec<PathBuf>,
    },
}

fn main() {
    let env = Env::default().filter_or("MY_LOG_LEVEL", "info");
    env_logger::init_from_env(env);
    log::info!("Starting ooxml-version-control");

    let cli = Cli::parse();

    match &cli.command {
        Commands::CheckIn { paths } => {
            for path in paths {
                if path.is_file() {
                    log::info!("Checking in: {:?}", path);

                    let file_name = path.file_name().unwrap().to_str().unwrap();
                    log::debug!("File name: {:?}", file_name);

                    let output_dir = path.with_file_name(file_name.to_owned() + "_ooxml");
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
                            .save();
                    }
                    let content_types = work_dir.join("[Content_Types].xml");
                    if content_types.exists() {
                        OoxmlBuffer::new(content_types.to_str().unwrap())
                            .remove_calc_chain_content_type_override()
                            .save();
                    }

                    // Drop volatile document metadata that changes frequently and
                    // creates noisy diffs/conflicts across environments.
                    let core_props = work_dir.join("docProps/core.xml");
                    if core_props.exists() {
                        OoxmlBuffer::new(core_props.to_str().unwrap())
                            .remove_volatile_core_properties()
                            .save();
                    }
                    let app_props = work_dir.join("docProps/app.xml");
                    if app_props.exists() {
                        OoxmlBuffer::new(app_props.to_str().unwrap())
                            .remove_volatile_app_properties()
                            .save();
                    }

                    // Get the shared strings
                    let ss_file = work_dir.join("xl/sharedStrings.xml");
                    let default_sst = shared_strings::Sst {
                        xmlns: String::from(
                            "http://schemas.openxmlformats.org/spreadsheetml/2006/main",
                        ),
                        count: String::from("0"),
                        unique_count: String::from("0"),
                        si: vec![],
                    };
                    let sst: shared_strings::Sst =
                        read_xml_file(ss_file.to_str().unwrap()).unwrap_or(default_sst);

                    // Only rewrite worksheet XML files; keep other OOXML parts byte-faithful.
                    let worksheet_xml_files =
                        filesystem::collect_files(&work_dir, "xl/worksheets/*.xml");
                    for xml_file in worksheet_xml_files {
                        log::debug!("Inlining shared strings in worksheet: {:?}", xml_file);
                        OoxmlBuffer::new(xml_file.to_str().unwrap())
                            .inline_shared_strings(&sst)
                            .save();
                    }

                    // Normalize all XML-like parts to keep check-in output deterministic
                    // and easier to diff/merge.
                    let xml_like_files =
                        filesystem::collect_files_by_extension(&work_dir, &["xml", "rels"]);
                    for xml_file in xml_like_files {
                        OoxmlBuffer::new(xml_file.to_str().unwrap()).save();
                    }

                    filesystem::copy_dir(&work_dir, &output_dir);

                    log::info!("Checked in: {:?}", output_dir);
                } else {
                    panic!("Error: Path is not a valid file: {:?}", path);
                }
            }
        }
        Commands::CheckOut { paths } => {
            for path in paths {
                if path.is_dir() {
                    log::info!("Checking out: {:?}", path);

                    let input_dir = path.file_name().unwrap().to_str().unwrap();
                    log::debug!("Input dir: {:?}", input_dir);

                    let output_file =
                        path.with_file_name(input_dir.to_string().replace("_ooxml", ""));
                    log::debug!("File name: {:?}", output_file);

                    filesystem::zip(&path, &output_file);

                    log::info!("Checked out: {:?}", output_file);
                } else {
                    panic!("Error: Path is not a valid file: {:?}", path);
                }
            }
        }
    }
}
