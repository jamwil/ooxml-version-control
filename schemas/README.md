# Schemas

This directory contains the runtime OOXML XSD bundle used by:

- `ooxml-version-control validate --mode spec`

## Layout

- `schemas/ooxml-xsd/transitional/`  
  ECMA-376 Transitional XML Schema set.
- `schemas/ooxml-xsd/strict/`  
  ECMA-376 Strict XML Schema set.

## Regenerate The Bundle

1. Place these ECMA ZIPs in `reference/`:
   - `ECMA-376-1_5th_edition_december_2016.zip`
   - `ECMA-376-4_5th_edition_december_2016.zip`
2. Run:

```bash
just bundle-xsd
```

This extracts:

- `OfficeOpenXML-XMLSchema-Strict.zip` (from Part 1)
- `OfficeOpenXML-XMLSchema-Transitional.zip` (from Part 4)

into `schemas/ooxml-xsd/`.

## Notes

- `reference/` is intentionally ignored in git; this `schemas/` tree is the committed runtime artifact.
- Spec mode currently skips parts that require full MCE (Part 5) preprocessing.
