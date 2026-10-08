set quiet
set positional-arguments

# Run all non-mutating checks used by CI.
check: fmt-check lint test docs
    printf 'check: ok\n'

# Check Rust formatting without rewriting files.
fmt-check:
    just _run fmt-check cargo fmt --check

# Treat Clippy warnings as failures across every target and feature.
lint:
    just _run lint cargo clippy --locked --all-targets --all-features -- -D warnings

# Run tests; extra arguments are passed to Cargo without word splitting.
test *args:
    just _run test cargo test --locked "$@"

# Check repository guides, contribution rules, and bundled skills.
docs:
    just _run docs rumdl check README.md GLOSSARY.md CONTRIBUTING.md docs skills

# Capture once, print diagnostics only on failure, and preserve the exit status.
[private]
[script("sh")]
_run label +command:
    set -eu
    label=$1
    shift
    log=$(mktemp "${TMPDIR:-/tmp}/zk-check.XXXXXX")
    trap 'rm -f "$log"' 0
    if "$@" >"$log" 2>&1; then
        printf '%s: ok\n' "$label"
    else
        status=$?
        printf '%s: failed (exit %s)\n' "$label" "$status" >&2
        cat "$log" >&2
        exit "$status"
    fi
