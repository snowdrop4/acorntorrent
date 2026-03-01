set SCRIPT_DIR (dirname (status --current-filename))
cd $SCRIPT_DIR

# ------------------------------------------------------------------------------
# Python
# ------------------------------------------------------------------------------

fish integration_tests/run_lints.fish; or exit 1

# ------------------------------------------------------------------------------
# Rust
# ------------------------------------------------------------------------------

cargo +nightly fmt; or exit 1
cargo +nightly clippy --fix --allow-dirty; or exit 1
