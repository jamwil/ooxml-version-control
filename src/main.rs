use clap::{Parser, Subcommand};
use ooxml_version_control::{copy_dir, unzip, zip};
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
    env_logger::init();
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

                    unzip(&path, &work_dir);
                    // TODO: Manipulation of files
                    copy_dir(&work_dir, &output_dir);

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

                    zip(&path, &output_file);
                    log::info!("Checked out: {:?}", output_file);
                } else {
                    panic!("Error: Path is not a valid file: {:?}", path);
                }
            }
        }
    }
}
