# ooxml-version-control

Diffable, mergeable version control for OOXML files.

## Core Commands

- `check-in <file>...`
  - Converts compiled OOXML bundles (for example `.xlsx`, `.docx`, `.pptx`) into normalized raw trees (`*_ooxml`).
- `check-out <dir_ooxml>...`
  - Converts raw OOXML trees (`*_ooxml`) back into compiled bundles (original extension preserved from the directory name).
- `validate [--mode <basic|spec>] [--profile <transitional|strict>] [paths...]`
  - `--mode basic` (default): strict XML syntax validation for OOXML parts (`.xml` and `.rels`).
  - `--mode spec`: validates XML syntax and then validates supported OOXML parts against bundled ECMA-376 XSDs in `schemas/ooxml-xsd/`.
  - `--profile` selects the schema set for spec mode (`transitional` default, or `strict`).
  - Parts using Markup Compatibility and Extensibility (MCE) constructs are currently skipped in `spec` mode (Part 5 preprocessing is not implemented yet).
  - Paths are required and can be files or directories.

## Hook Setup

Install helper hooks once in your repository:

```bash
ooxml-version-control git-install --repo .
```

Installed hooks:

- `.git/hooks/pre-commit` -> reminds you to run `check-in` with explicit OOXML bundle paths.
- `.git/hooks/post-checkout` -> reminds you to run `check-out` with explicit `*_ooxml` paths if needed.
- `.git/hooks/post-merge` -> reminds you to run `check-out` with explicit `*_ooxml` paths if needed.

Use `--force` to overwrite existing hook files:

```bash
ooxml-version-control git-install --repo . --force
```

## Recommended Daily Flow

1. Open/edit your OOXML file as usual (for example `file.xlsx`, `file.docx`, or `file.pptx`).
2. Run `check-in path/to/file.ext` for each bundle you want to convert.
3. Commit your desired files.
4. Run `check-out path/to/file.ext_ooxml` when you need a compiled bundle from raw OOXML.

## Notes

- Hooks call the same compiled binary path used during installation.
- If hooks do not run, ensure they are executable and your git config allows local hooks.
- Conversion commands require explicit paths.
