# Run the tests with coverage
test:
    # Requires cargo-tarpaulin to be installed
    cargo tarpaulin

# Format the unbundled OOXML files in the fixtures directory
format-fixtures:
    # Requires prettier and the official prettier-xml plugin to be installed
    npx prettier --plugin=@prettier/plugin-xml \
        --xml-whitespace-sensitivity preserve \
        --parser xml --write 'tests/fixtures/**/*.{rels,xml}'
    # Requires https://github.com/jurosh/nodejs-eol-converter-cli
    npx eolConverter crlf 'tests/fixtures/*/**/*.{rels,xml}'

# Validate the bundled OOXML files in the fixtures directory
validate-fixtures:
    # Requires https://github.com/mikeebowen/OOXML-Validator to be built and in the PATH
    OOXMLValidatorCLI tests/fixtures/

# Extract bundled OOXML XSDs from ECMA zip files in reference/
bundle-xsd:
    mkdir -p schemas/ooxml-xsd/strict schemas/ooxml-xsd/transitional
    find schemas/ooxml-xsd/strict -type f -delete
    find schemas/ooxml-xsd/transitional -type f -delete
    tmpdir=$$(mktemp -d); \
    unzip -q reference/ECMA-376-1_5th_edition_december_2016.zip OfficeOpenXML-XMLSchema-Strict.zip -d "$$tmpdir"; \
    unzip -q reference/ECMA-376-4_5th_edition_december_2016.zip OfficeOpenXML-XMLSchema-Transitional.zip -d "$$tmpdir"; \
    unzip -q "$$tmpdir/OfficeOpenXML-XMLSchema-Strict.zip" -d schemas/ooxml-xsd/strict; \
    unzip -q "$$tmpdir/OfficeOpenXML-XMLSchema-Transitional.zip" -d schemas/ooxml-xsd/transitional
