use clap::{Parser, Subcommand};
use env_logger::{self, Env};
use ooxml_version_control::filesystem;
use ooxml_version_control::ooxml::{OoxmlBuffer, read_xml_file};
use ooxml_version_control::ooxml::schemas::shared_strings;
use std::path::PathBuf;
use std::fs::remove_file;
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

                    // Get the shared strings
                    let ss_file = work_dir.join("xl/sharedStrings.xml");
                    let sst: shared_strings::Sst =
                        read_xml_file(ss_file.to_str().unwrap()).unwrap();

                    // Process all XML files in work directory
                    let xml_files = filesystem::collect_files(&work_dir, "**/*.xml");
                    for xml_file in xml_files {
                        log::debug!("Tidying XML file: {:?}", xml_file);
                        OoxmlBuffer::new(xml_file.to_str().unwrap())
                            .tidy()
                            .inline_shared_strings(&sst)
                            .save();
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
