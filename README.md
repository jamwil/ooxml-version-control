# ooxml-version-control

Diffable, mergeable version control for OOXML files.

Status: Alpha software. Expect breaking changes and validate outputs in your workflow before production use.

CLI binary name:
- `ocv`

## Core Commands

- `check-in [-o <container-dir>] <file>...`
  - Converts compiled OOXML bundles (for example `.xlsx`, `.docx`, `.pptx`) into normalized raw trees (`*_ooxml`).
  - For `check-in`, `-o, --output` is a container directory. The generated raw tree directory is created inside it.
  - Raw tree directory naming is app-controlled (`<input-filename>_ooxml`), which `check-out` uses to derive the compiled output extension when `-o` is not provided.
- `check-out [-o <output>] <dir_ooxml>...`
  - Converts raw OOXML trees (`*_ooxml`) back into compiled bundles (original extension preserved from the directory name).
  - For `check-out`, `-o, --output` is the exact destination compiled file path (for example `report.xlsx`).
  - `-o, --output` requires exactly one input path.
- `validate [--mode <basic|spec>] [--profile <transitional|strict>] [paths...]`
  - `--mode basic` (default): strict XML syntax validation for OOXML parts (`.xml` and `.rels`).
  - `--mode spec`: validates XML syntax and then validates supported OOXML parts against bundled ECMA-376 XSDs in `schemas/ooxml-xsd/`.
  - `--profile` selects the schema set for spec mode (`transitional` default, or `strict`).
  - Parts using Markup Compatibility and Extensibility (MCE) constructs are currently skipped in `spec` mode (Part 5 preprocessing is not implemented yet).
  - Paths are required and can be files or directories.

## Hook Setup

Install reminder-only hooks once in your repository:

```bash
ocv git-install --repo .
```

Installed hooks:

- `.git/hooks/pre-commit` -> reminds you to run `check-in` with explicit OOXML bundle paths.
- `.git/hooks/post-checkout` -> reminds you to run `check-out` with explicit `*_ooxml` paths if needed.
- `.git/hooks/post-merge` -> reminds you to run `check-out` with explicit `*_ooxml` paths if needed.

These hooks only print `echo` reminders. They do not invoke `ocv check-in` or `ocv check-out` automatically.

Use `--force` to overwrite existing hook files:

```bash
ocv git-install --repo . --force
```

## Recommended Daily Flow

1. Open/edit your OOXML file as usual (for example `file.xlsx`, `file.docx`, or `file.pptx`).
2. Run `check-in path/to/file.ext` for each bundle you want to convert.
3. Commit your desired files.
4. Run `check-out path/to/file.ext_ooxml` when you need a compiled bundle from raw OOXML.

Example explicit output paths:

```bash
ocv check-in -o path/to/raw path/to/my_bundle.xlsx
ocv check-out -o path/to/build/my_bundle.xlsx path/to/raw/my_bundle.xlsx_ooxml
```

## Notes

- Hooks call the same compiled binary path used during installation.
- `git-install` creates reminder hooks only; it does not automate conversion commands.
- If hooks do not run, ensure they are executable and your git config allows local hooks.
- Conversion commands require explicit paths.
- Binary OOXML parts (for example macro payloads like `vbaProject.bin`) are passed through unchanged; validation only checks `.xml` and `.rels` parts.
