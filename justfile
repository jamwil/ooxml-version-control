# Run the tests with coverage
test:
    # Requires cargo-tarpaulin to be installed
    cargo tarpaulin --include-tests --fail-under 100 --follow-exec

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
