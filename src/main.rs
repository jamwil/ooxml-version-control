use clap::{Parser, Subcommand};
use std::path::PathBuf;

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
    let cli = Cli::parse();

    match &cli.command {
        Commands::CheckIn { paths } => {
            for path in paths {
                if path.is_file() {
                    println!("Checking in: {:?}", path);
                } else {
                    panic!("Error: Path is not a valid file: {:?}", path);
                }
            }
        }
        Commands::CheckOut { paths } => {
            for path in paths {
                if path.is_file() {
                    println!("Checking out: {:?}", path);
                } else {
                    panic!("Error: Path is not a valid file: {:?}", path);
                }
            }
        }
    }
}
