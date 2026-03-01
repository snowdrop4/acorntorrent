set SCRIPT_DIR (dirname (status --current-filename))
cd $SCRIPT_DIR

uv run ruff format .; or exit 1
uv run ruff check --fix --unsafe-fixes .; or exit 1
