# ooxml-version-control

Diffable, mergeable version control for OOXML files.

## Core Commands

- `check-in <file.xlsx>...`
  - Converts compiled OOXML bundles (`.xlsx`) into normalized raw trees (`*_ooxml`).
- `check-out <dir.xlsx_ooxml>...`
  - Converts raw OOXML trees (`*_ooxml`) back into compiled bundles (`.xlsx`).
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

- `.git/hooks/pre-commit` -> reminds you to run `check-in` with explicit `.xlsx` paths.
- `.git/hooks/post-checkout` -> reminds you to run `check-out` with explicit `*_ooxml` paths if needed.
- `.git/hooks/post-merge` -> reminds you to run `check-out` with explicit `*_ooxml` paths if needed.

Use `--force` to overwrite existing hook files:

```bash
ooxml-version-control git-install --repo . --force
```

## Recommended Daily Flow

1. Open/edit `file.xlsx` as usual.
2. Run `check-in path/to/file.xlsx` for each workbook you want to convert.
3. Commit your desired files.
4. Run `check-out path/to/file.xlsx_ooxml` when you need a compiled workbook from raw OOXML.

## Notes

- Hooks call the same compiled binary path used during installation.
- If hooks do not run, ensure they are executable and your git config allows local hooks.
- Conversion commands require explicit paths.
