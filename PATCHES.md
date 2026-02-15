# Dependency Patches

## `quick-xml`

- Crate: `quick-xml`
- Reason: Required whitespace-handling behavior in serde deserialization for OOXML use cases.
- Override mechanism: Cargo `[patch.crates-io]` in `Cargo.toml`
- Source: `https://github.com/jamwil/quick-xml.git`
- Pinned revision: `399d550536f6c74ec5e824b85cb44706a4fa0837` (`space-issue`)

### Maintenance

- Keep the override pinned to an immutable commit (not a moving branch).
- Track upstream fix status and remove the patch when a crates.io release includes the required behavior.
