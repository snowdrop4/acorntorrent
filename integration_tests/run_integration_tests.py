import subprocess
import sys
from pathlib import Path


def build_docker_images(compose_files: list[Path]) -> tuple[int, str, str]:
    print("Building Docker images...")

    # Use the first compose file to build all images (they're all the same)
    if not compose_files:
        return False

    result = subprocess.run(
        [
            "docker-compose",
            "-f",
            str(compose_files[0]),
            "build",
            "--no-cache",
        ],
        capture_output=True,
        text=True,
    )

    return result.returncode, result.stdout, result.stderr


def main() -> None:
    compose_dir = Path(__file__).parent / "compose-files"
    results_dir = Path(__file__).parent / "test-results"

    results_dir.mkdir(exist_ok=True)

    compose_files = sorted(compose_dir.glob("docker-compose-*.yml"))

    if len(compose_files) == 0:
        print("No compose files found")
        sys.exit(1)

    # Build images once before running tests
    returncode, stdout, stderr = build_docker_images(compose_files)

    if returncode == 0:
        print("Images built successfully")
    else:
        print("Failed to build images")

        print("-" * 80)
        print("Stdout:")
        print("-" * 80)
        print(stdout)

        print()
        print("-" * 80)
        print("Stderr:")
        print("-" * 80)
        print(stderr)

        exit(1)

    print(f"Running {len(compose_files)} tests\n")

    total = 0
    passed = 0
    failed = 0

    for compose_file in compose_files:
        test_name = compose_file.stem

        print(f"\n{'=' * 60}")
        print(f"Running: {test_name}")
        print("=" * 60)

        result = subprocess.run(
            [
                "docker-compose",
                "-f",
                str(compose_file),
                "up",
                "--abort-on-container-exit",
            ],
        )

        if result.returncode == 0:
            print("✓ PASSED")
            passed += 1
        else:
            print("✗ FAILED")
            failed += 1

        subprocess.run(
            ["docker-compose", "-f", str(compose_file), "down", "-v"],
        )

        total += 1

    print(f"\nTotal: {total}  Passed: {passed}  Failed: {failed}")

    sys.exit(0 if failed == 0 else 1)


if __name__ == "__main__":
    main()
