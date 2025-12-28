import hashlib
import subprocess
from pathlib import Path


def create_test_file(
    filepath: Path,
    size_mib: int = 10,
) -> str:
    print(f"Creating test file: {filepath} ({size_mib}MiB)")
    filepath.parent.mkdir(parents=True, exist_ok=True)

    # Create file with deterministic data
    with open(filepath, "wb") as f:
        chunk = b"A" * 1024 * 1024  # 1MiB chunk
        for _ in range(size_mib):
            f.write(chunk)

    # Calculate SHA1 hash
    sha1 = hashlib.sha1()
    with open(filepath, "rb") as f:
        while chunk := f.read(8192):
            sha1.update(chunk)

    print(f"Test file created. SHA1: {sha1.hexdigest()}")
    return sha1.hexdigest()


def create_torrent_file(
    source_file: Path,
    torrent_file: Path,
    tracker_url: str,
) -> tuple[int, str, str]:
    print(f"Creating torrent file: {torrent_file}")
    torrent_file.parent.mkdir(parents=True, exist_ok=True)

    result = subprocess.run(
        [
            "transmission-create",
            "-o",
            str(torrent_file),
            "-t",
            tracker_url,
            str(source_file),
        ],
        capture_output=True,
        text=True,
    )

    return result.returncode, result.stdout, result.stderr


def main() -> None:
    base_dir = Path(__file__).parent
    test_data_dir = base_dir / "compose-files" / "test-data"

    # Create test file
    test_file = test_data_dir / "seed-file.dat"
    create_test_file(test_file, size_mib=1)

    torrents_dir = test_data_dir / "torrents"

    for tracker in ["chihaya", "opentracker"]:
        tracker_url = f"http://{tracker}:6969/announce"
        torrent_file = torrents_dir / f"test-{tracker}.torrent"

        returncode, stdout, stderr = create_torrent_file(
            test_file,
            torrent_file,
            tracker_url,
        )

        if returncode == 0:
            print(f"Torrent for {tracker} created successfully")
            exit(0)
        else:
            print(f"Error creating torrent for tracker: {tracker}")
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


if __name__ == "__main__":
    main()
