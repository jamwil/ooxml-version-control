use clap::{Parser, Subcommand};
use env_logger::{self, Env};
use ooxml_version_control::filesystem;
use ooxml_version_control::ooxml::schemas::shared_strings;
use ooxml_version_control::ooxml::{read_xml_file, OoxmlBuffer};
use std::collections::BTreeSet;
use std::fs;
use std::fs::remove_file;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use tempfile::tempdir;
use walkdir::WalkDir;

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
    /// Convert compiled .xlsx files to raw *_ooxml trees
    VcsIn {
        /// Files to convert. If omitted, auto-discovers .xlsx files.
        paths: Vec<PathBuf>,
        /// Stage *_ooxml output and unstage compiled .xlsx files
        #[arg(long)]
        stage: bool,
    },
    /// Convert raw *_ooxml trees to compiled .xlsx files
    VcsOut {
        /// Directories to convert. If omitted, auto-discovers tracked *_ooxml trees.
        paths: Vec<PathBuf>,
    },
    /// Install git hooks for a raw-commit / compiled-worktree workflow
    GitInstall {
        /// Repository path where hooks should be installed
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        /// Overwrite existing hooks
        #[arg(long)]
        force: bool,
    },
}

fn check_in_path(path: &PathBuf) {
    if !path.is_file() {
        panic!("Error: Path is not a valid file: {:?}", path);
    }

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
            .save();
    }

    // Normalize all XML-like parts to keep check-in output deterministic
    // and easier to diff/merge.
    let xml_like_files = filesystem::collect_files_by_extension(&work_dir, &["xml", "rels"]);
    for xml_file in xml_like_files {
        OoxmlBuffer::new(xml_file.to_str().unwrap()).save();
    }

    filesystem::copy_dir(&work_dir, &output_dir);

    log::info!("Checked in: {:?}", output_dir);
}

fn check_out_path(path: &PathBuf) {
    if !path.is_dir() {
        panic!("Error: Path is not a valid file: {:?}", path);
    }

    log::info!("Checking out: {:?}", path);

    let input_dir = path.file_name().unwrap().to_str().unwrap();
    log::debug!("Input dir: {:?}", input_dir);

    let output_file = path.with_file_name(input_dir.to_string().replace("_ooxml", ""));
    log::debug!("File name: {:?}", output_file);

    filesystem::zip(&path, &output_file);

    log::info!("Checked out: {:?}", output_file);
}

fn discover_xlsx_files() -> Vec<PathBuf> {
    WalkDir::new(".")
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_file())
        .filter(|entry| entry.file_name().to_str().unwrap_or("").ends_with(".xlsx"))
        .filter(|entry| {
            !entry
                .path()
                .components()
                .any(|c| c.as_os_str() == ".git" || c.as_os_str() == "target")
        })
        .map(|entry| entry.path().to_path_buf())
        .collect()
}

fn git_output(args: &[&str], cwd: &PathBuf) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()
}

fn find_repo_root(path: &PathBuf) -> Option<PathBuf> {
    let output = git_output(&["rev-parse", "--show-toplevel"], path)?;
    Some(PathBuf::from(output.trim()))
}

fn discover_tracked_ooxml_dirs(base: &PathBuf) -> Vec<PathBuf> {
    let mut dirs = BTreeSet::new();
    let Some(files) = git_output(&["ls-files"], base) else {
        return vec![];
    };
    for line in files.lines() {
        let path = PathBuf::from(line);
        if let Some(component) = path
            .components()
            .find(|c| c.as_os_str().to_str().unwrap_or("").ends_with("_ooxml"))
        {
            let component = component.as_os_str().to_string_lossy().to_string();
            if let Some(idx) = line.find(&component) {
                dirs.insert(base.join(&line[..idx + component.len()]));
            }
        }
    }
    dirs.into_iter().collect()
}

fn discover_staged_xlsx_files(base: &PathBuf) -> Vec<PathBuf> {
    let mut files = vec![];
    let Some(output) = git_output(
        &["diff", "--cached", "--name-only", "--diff-filter=ACMR"],
        base,
    ) else {
        return files;
    };
    for line in output.lines() {
        if line.ends_with(".xlsx") {
            files.push(base.join(line));
        }
    }
    files
}

fn vcs_in(paths: &[PathBuf], stage: bool) {
    if stage {
        let cwd = PathBuf::from(".");
        let Some(repo_root) = find_repo_root(&cwd) else {
            panic!("Error: --stage requires running inside a git repository");
        };

        let stage_targets: Vec<PathBuf> = if paths.is_empty() {
            discover_staged_xlsx_files(&repo_root)
        } else {
            paths.to_vec()
        };

        for path in &stage_targets {
            check_in_path(path);
        }

        for path in stage_targets {
            let file_name = path.file_name().unwrap().to_str().unwrap();
            let output_dir = path.with_file_name(file_name.to_owned() + "_ooxml");

            let add_status = Command::new("git")
                .arg("add")
                .arg("--all")
                .arg("--")
                .arg(&output_dir)
                .current_dir(&repo_root)
                .status()
                .unwrap();
            if !add_status.success() {
                panic!("Error: failed to stage {:?}", output_dir);
            }

            let _ = Command::new("git")
                .arg("reset")
                .arg("-q")
                .arg("--")
                .arg(&path)
                .current_dir(&repo_root)
                .status();
        }
    } else {
        let discovered_paths = if paths.is_empty() {
            discover_xlsx_files()
        } else {
            paths.to_vec()
        };

        for path in discovered_paths {
            check_in_path(&path);
        }
    }
}

fn vcs_out(paths: &[PathBuf]) {
    let discovered_paths = if paths.is_empty() {
        discover_tracked_ooxml_dirs(&PathBuf::from("."))
    } else {
        paths.to_vec()
    };
    for path in discovered_paths {
        check_out_path(&path);
    }
}

fn install_hook(path: &PathBuf, content: &str, force: bool) {
    if path.exists() && !force {
        log::warn!("Skipping existing hook (use --force): {:?}", path);
        return;
    }
    let mut file = fs::File::create(path).unwrap();
    file.write_all(content.as_bytes()).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms).unwrap();
    }
    log::info!("Installed hook: {:?}", path);
}

fn git_install(repo: &PathBuf, force: bool) {
    let repo_root =
        find_repo_root(repo).unwrap_or_else(|| panic!("Error: {:?} is not a git repository", repo));
    let hooks_dir = repo_root.join(".git/hooks");
    fs::create_dir_all(&hooks_dir).unwrap();
    let exe = std::env::current_exe().unwrap();
    let exe_display = exe.to_string_lossy();

    let pre_commit = format!("#!/bin/sh\nset -e\n\"{}\" vcs-in --stage\n", exe_display);
    install_hook(&hooks_dir.join("pre-commit"), &pre_commit, force);

    let post_checkout = format!("#!/bin/sh\nset -e\n\"{}\" vcs-out\n", exe_display);
    install_hook(&hooks_dir.join("post-checkout"), &post_checkout, force);

    let post_merge = format!("#!/bin/sh\nset -e\n\"{}\" vcs-out\n", exe_display);
    install_hook(&hooks_dir.join("post-merge"), &post_merge, force);
}

fn main() {
    let env = Env::default().filter_or("MY_LOG_LEVEL", "info");
    env_logger::init_from_env(env);
    log::info!("Starting ooxml-version-control");

    let cli = Cli::parse();

    match &cli.command {
        Commands::CheckIn { paths } => {
            for path in paths {
                check_in_path(path);
            }
        }
        Commands::CheckOut { paths } => {
            for path in paths {
                check_out_path(path);
            }
        }
        Commands::VcsIn { paths, stage } => vcs_in(paths, *stage),
        Commands::VcsOut { paths } => vcs_out(paths),
        Commands::GitInstall { repo, force } => git_install(repo, *force),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::panic;
    use std::sync::{Mutex, OnceLock};
    use tempfile::tempdir;

    fn cwd_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn lock_cwd() -> std::sync::MutexGuard<'static, ()> {
        cwd_lock().lock().unwrap_or_else(|e| e.into_inner())
    }

    struct CwdGuard {
        original: PathBuf,
    }

    impl CwdGuard {
        fn to(path: &PathBuf) -> Self {
            let original = std::env::current_dir().unwrap();
            std::env::set_current_dir(path).unwrap();
            Self { original }
        }
    }

    impl Drop for CwdGuard {
        fn drop(&mut self) {
            let _ = std::env::set_current_dir(&self.original);
        }
    }

    fn init_git_repo(dir: &PathBuf) {
        assert!(Command::new("git")
            .arg("init")
            .arg(dir)
            .status()
            .unwrap()
            .success());
    }

    #[test]
    fn test_discover_xlsx_files_filters_git_and_target() {
        let _lock = lock_cwd();
        let temp = tempdir().unwrap();
        let root = temp.path().to_path_buf();
        let _guard = CwdGuard::to(&root);

        fs::create_dir_all(root.join(".git")).unwrap();
        fs::create_dir_all(root.join("target")).unwrap();
        fs::create_dir_all(root.join("docs")).unwrap();

        fs::write(root.join("keep.xlsx"), b"x").unwrap();
        fs::write(root.join(".git/skip.xlsx"), b"x").unwrap();
        fs::write(root.join("target/skip.xlsx"), b"x").unwrap();
        fs::write(root.join("docs/readme.txt"), b"x").unwrap();

        let mut files = discover_xlsx_files();
        files.sort();

        assert_eq!(files, vec![PathBuf::from("./keep.xlsx")]);
    }

    #[test]
    fn test_git_output_and_find_repo_root_fail_outside_repo() {
        let _lock = lock_cwd();
        let temp = tempdir().unwrap();
        let root = temp.path().to_path_buf();
        let _guard = CwdGuard::to(&root);

        assert!(git_output(&["rev-parse", "--show-toplevel"], &root).is_none());
        assert!(find_repo_root(&root).is_none());
    }

    #[test]
    fn test_discover_tracked_ooxml_dirs_returns_empty_outside_repo() {
        let temp = tempdir().unwrap();
        let root = temp.path().to_path_buf();

        let dirs = discover_tracked_ooxml_dirs(&root);
        assert!(dirs.is_empty());
    }

    #[test]
    fn test_discover_tracked_ooxml_dirs_in_repo() {
        let temp = tempdir().unwrap();
        let root = temp.path().to_path_buf();
        init_git_repo(&root);

        let tracked = root.join("book.xlsx_ooxml/xl/workbook.xml");
        fs::create_dir_all(tracked.parent().unwrap()).unwrap();
        fs::write(&tracked, b"<x/>").unwrap();
        fs::write(root.join("notes.txt"), b"n").unwrap();

        assert!(Command::new("git")
            .arg("add")
            .arg("--all")
            .current_dir(&root)
            .status()
            .unwrap()
            .success());

        let dirs = discover_tracked_ooxml_dirs(&root);
        assert_eq!(dirs, vec![root.join("book.xlsx_ooxml")]);
    }

    #[test]
    fn test_discover_staged_xlsx_files() {
        let temp = tempdir().unwrap();
        let root = temp.path().to_path_buf();
        init_git_repo(&root);

        fs::write(root.join("a.xlsx"), b"a").unwrap();
        fs::write(root.join("b.txt"), b"b").unwrap();
        assert!(Command::new("git")
            .arg("add")
            .arg("a.xlsx")
            .arg("b.txt")
            .current_dir(&root)
            .status()
            .unwrap()
            .success());

        let files = discover_staged_xlsx_files(&root);
        assert_eq!(files, vec![root.join("a.xlsx")]);
    }

    #[test]
    fn test_vcs_in_no_paths_no_stage() {
        let _lock = lock_cwd();
        let temp = tempdir().unwrap();
        let root = temp.path().to_path_buf();
        let _guard = CwdGuard::to(&root);

        vcs_in(&[], false);
    }

    #[test]
    fn test_vcs_in_stage_with_no_paths_uses_staged_files_only() {
        let _lock = lock_cwd();
        let temp = tempdir().unwrap();
        let root = temp.path().to_path_buf();
        init_git_repo(&root);
        let _guard = CwdGuard::to(&root);

        let fixture =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/simple_book.xlsx");
        let workbook = root.join("book.xlsx");
        let untouched = root.join("untouched.xlsx");
        fs::copy(fixture, &workbook).unwrap();
        fs::copy(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/simple_book.xlsx"),
            &untouched,
        )
        .unwrap();

        assert!(Command::new("git")
            .arg("add")
            .arg("book.xlsx")
            .current_dir(&root)
            .status()
            .unwrap()
            .success());

        vcs_in(&[], true);

        assert!(root.join("book.xlsx_ooxml").is_dir());
        assert!(!root.join("untouched.xlsx_ooxml").exists());
    }

    #[test]
    fn test_vcs_in_stage_requires_git_repo() {
        let _lock = lock_cwd();
        let temp = tempdir().unwrap();
        let root = temp.path().to_path_buf();
        let _guard = CwdGuard::to(&root);

        let result = panic::catch_unwind(|| vcs_in(&[], true));
        assert!(result.is_err());
    }

    #[test]
    fn test_vcs_in_stage_panics_when_git_add_fails() {
        let _lock = lock_cwd();
        let temp = tempdir().unwrap();
        let root = temp.path().to_path_buf();
        init_git_repo(&root);
        let _guard = CwdGuard::to(&root);

        let fixture =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/simple_book.xlsx");
        let workbook = root.join("book.xlsx");
        fs::copy(fixture, &workbook).unwrap();

        fs::write(root.join(".git/index.lock"), b"lock").unwrap();

        let result = panic::catch_unwind(|| vcs_in(&[workbook], true));
        assert!(result.is_err());
    }

    #[test]
    fn test_vcs_out_no_paths_uses_tracked_dirs() {
        let _lock = lock_cwd();
        let temp = tempdir().unwrap();
        let root = temp.path().to_path_buf();
        init_git_repo(&root);
        let _guard = CwdGuard::to(&root);

        let ooxml = root.join("book.xlsx_ooxml");
        fs::create_dir_all(&ooxml).unwrap();
        fs::write(ooxml.join("doc.xml"), b"<a/>").unwrap();
        assert!(Command::new("git")
            .arg("add")
            .arg("--all")
            .current_dir(&root)
            .status()
            .unwrap()
            .success());

        vcs_out(&[]);

        assert!(root.join("book.xlsx").is_file());
    }

    #[test]
    fn test_install_hook_skip_and_force() {
        let temp = tempdir().unwrap();
        let hook_path = temp.path().join("hook.sh");

        install_hook(&hook_path, "first\n", false);
        let result = panic::catch_unwind(|| install_hook(&hook_path, "second\n", false));
        assert!(result.is_ok());
        let content = fs::read_to_string(&hook_path).unwrap();
        assert_eq!(content, "first\n");

        install_hook(&hook_path, "second\n", true);
        let content = fs::read_to_string(&hook_path).unwrap();
        assert_eq!(content, "second\n");
    }
}
