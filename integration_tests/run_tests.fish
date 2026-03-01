set SCRIPT_DIR (dirname (status --current-filename))
cd $SCRIPT_DIR

uv run python generate_test_data.py; or exit 1
uv run python generate_test_matrix.py; or exit 1
uv run python run_integration_tests.py; or exit 1
