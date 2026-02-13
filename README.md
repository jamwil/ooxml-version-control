# ooxml-version-control

Diffable, mergeable version control for OOXML files.

## Core Commands

- `check-in <file.xlsx>...`
  - Converts compiled OOXML bundles (`.xlsx`) into normalized raw trees (`*_ooxml`).
- `check-out <dir.xlsx_ooxml>...`
  - Converts raw OOXML trees (`*_ooxml`) back into compiled bundles (`.xlsx`).

## Git Ergonomic Workflow

The repository can be configured so that:

- You edit and open the compiled `.xlsx` file in your working tree.
- Git commits only the raw `*_ooxml` directory representation.
- Compiled `.xlsx` files are automatically regenerated after branch changes.

### Commands for this workflow

- `vcs-in [paths...]`
  - Runs check-in conversion from `.xlsx` to `*_ooxml`.
  - If no paths are given, it auto-discovers `.xlsx` files in the working tree.
- `vcs-in --stage [paths...]`
  - Converts as above, then:
  - Stages `*_ooxml` changes.
  - Unstages the corresponding `.xlsx` paths.
  - If no paths are provided, it only processes `.xlsx` files already staged in git.
- `vcs-out [paths...]`
  - Runs check-out conversion from `*_ooxml` to `.xlsx`.
  - If no paths are given, it discovers tracked `*_ooxml` trees via `git ls-files`.
- `git-install [--repo <path>] [--force]`
  - Installs git hooks for automatic sync behavior.
- `validate [--mode <basic|spec>] [--profile <transitional|strict>] [paths...]`
  - `--mode basic` (default): strict XML syntax validation for OOXML parts (`.xml` and `.rels`).
  - `--mode spec`: validates XML syntax and then validates supported OOXML parts against bundled ECMA-376 XSDs in `schemas/ooxml-xsd/`.
  - `--profile` selects the schema set for spec mode (`transitional` default, or `strict`).
  - Parts using Markup Compatibility and Extensibility (MCE) constructs are currently skipped in `spec` mode (Part 5 preprocessing is not implemented yet).
  - Paths can be files or directories.
  - If no paths are provided, it validates XML files under tracked `*_ooxml` trees.

## Hook Setup

Install hooks once in your repository:

```bash
ooxml-version-control git-install --repo .
```

Installed hooks:

- `.git/hooks/pre-commit` -> runs `vcs-in --stage`
- `.git/hooks/post-checkout` -> runs `vcs-out`
- `.git/hooks/post-merge` -> runs `vcs-out`

Use `--force` to overwrite existing hook files:

```bash
ooxml-version-control git-install --repo . --force
```

## Recommended Daily Flow

1. Open/edit `file.xlsx` as usual.
2. `git add file.xlsx` for files you deliberately want included in this commit.
3. `git commit ...`
4. Pre-commit converts staged `.xlsx` paths to `*_ooxml`, stages the raw tree, and unstages the compiled `.xlsx`.
5. After checkout/merge, hooks regenerate `.xlsx` files in your working tree.

## Notes

- Hooks call the same compiled binary path used during installation.
- If hooks do not run, ensure they are executable and your git config allows local hooks.
- `check-in` and `check-out` remain available for explicit/manual conversions.
