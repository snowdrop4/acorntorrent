"""
Test orchestrator that runs inside Docker to coordinate integration tests.
"""

import argparse
import hashlib
import os
import socket
import subprocess
import sys
import time
from pathlib import Path
from urllib.error import HTTPError, URLError
from urllib.request import urlopen

from colorama import Fore, Style


def wait_for_tracker(tracker_url: str, timeout: int = 30) -> bool:
    print(f"{Fore.BLUE}Waiting for tracker at {tracker_url}...{Style.RESET_ALL}")
    start = time.time()

    while time.time() - start < timeout:
        try:
            # Try to connect to tracker announce endpoint
            # We just need to check if the server is responding
            with urlopen(tracker_url, timeout=2):
                # We expect an error response (missing params), but that means it's up
                print(f"{Fore.GREEN}Tracker is ready!{Style.RESET_ALL}")
                return True
        except HTTPError as e:
            # If we get an HTTPError with a proper HTTP error code, the server is up
            # (it just doesn't like our request parameters, which is expected)
            if e.code in [400, 403, 404, 405]:
                print(f"{Fore.GREEN}Tracker is ready!{Style.RESET_ALL}")
                return True
            # Otherwise, the server might not be ready yet
            pass
        except URLError:
            # Connection refused or other network error - server not ready
            pass
        except socket.timeout:
            pass
        except Exception:
            pass
        time.sleep(1)

    print(f"{Fore.RED}Tracker failed to start in time{Style.RESET_ALL}")
    return False


def create_test_file(filepath: Path, size_mb: int = 10) -> str:
    print(f"{Fore.BLUE}Creating test file: {filepath} ({size_mb}MB){Style.RESET_ALL}")
    filepath.parent.mkdir(parents=True, exist_ok=True)

    # Create file with pseudo-random but deterministic data
    with open(filepath, "wb") as f:
        chunk = b"A" * 1024 * 1024  # 1MB chunk
        for i in range(size_mb):
            f.write(chunk)

    # Calculate SHA1 hash
    sha1 = hashlib.sha1()
    with open(filepath, "rb") as f:
        while chunk := f.read(8192):
            sha1.update(chunk)

    print(f"{Fore.GREEN}Test file created. SHA1: {sha1.hexdigest()}{Style.RESET_ALL}")
    return sha1.hexdigest()


def create_torrent_file(source_file: Path, torrent_file: Path, tracker_url: str) -> bool:
    print(f"{Fore.BLUE}Creating torrent file: {torrent_file}{Style.RESET_ALL}")

    # Use transmission-create if available, otherwise mktorrent
    try:
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

        if result.returncode == 0:
            print(f"{Fore.GREEN}Torrent file created successfully{Style.RESET_ALL}")
            return True
        else:
            print(f"{Fore.RED}Failed to create torrent: {result.stderr}{Style.RESET_ALL}")
            return False
    except FileNotFoundError:
        print(f"{Fore.YELLOW}transmission-create not found, trying mktorrent{Style.RESET_ALL}")
        try:
            result = subprocess.run(
                [
                    "mktorrent",
                    "-a",
                    tracker_url,
                    "-o",
                    str(torrent_file),
                    str(source_file),
                ],
                capture_output=True,
                text=True,
            )

            if result.returncode == 0:
                print(f"{Fore.GREEN}Torrent file created successfully{Style.RESET_ALL}")
                return True
            else:
                print(f"{Fore.RED}Failed to create torrent: {result.stderr}{Style.RESET_ALL}")
                return False
        except FileNotFoundError:
            print(f"{Fore.RED}Neither transmission-create nor mktorrent found{Style.RESET_ALL}")
            return False


def test_download_scenario(tracker: str, client: str, tracker_url: str) -> bool:
    """
    Test scenario: Other client seeds, AcornTorrent downloads.
    """
    print(f"{Fore.BLUE}\n=== DOWNLOAD SCENARIO ==={Style.RESET_ALL}")
    print(f"{Fore.BLUE}Tracker: {tracker}, Client: {client}{Style.RESET_ALL}")

    # Use pre-generated test data
    test_file = Path("/test-data/seed-file.dat")
    torrent_file = Path(f"/test-data/torrents/test-{tracker}.torrent")

    # Verify test files exist
    if not test_file.exists():
        print(f"{Fore.RED}Test file not found: {test_file}{Style.RESET_ALL}")
        print(f"{Fore.RED}Please run generate_test_data.py before running tests{Style.RESET_ALL}")
        return False

    if not torrent_file.exists():
        print(f"{Fore.RED}Torrent file not found: {torrent_file}{Style.RESET_ALL}")
        print(f"{Fore.RED}Please run generate_test_data.py before running tests{Style.RESET_ALL}")
        return False

    print(f"{Fore.BLUE}Using test file: {test_file}{Style.RESET_ALL}")
    print(f"{Fore.BLUE}Using torrent file: {torrent_file}{Style.RESET_ALL}")

    # 3. Start seeding with other client
    print(f"{Fore.BLUE}Starting {client} as seeder...{Style.RESET_ALL}")
    # TODO: Add client-specific seeding commands

    # 4. Start AcornTorrent download
    print(f"{Fore.BLUE}Starting AcornTorrent download...{Style.RESET_ALL}")
    # TODO: Invoke AcornTorrent to download the file

    # 5. Verify download
    print(f"{Fore.BLUE}Verifying download...{Style.RESET_ALL}")
    # TODO: Check that file was downloaded and hash matches

    print(f"{Fore.YELLOW}Download test NOT IMPLEMENTED YET{Style.RESET_ALL}")
    return False


def test_upload_scenario(tracker: str, client: str, tracker_url: str) -> bool:
    """
    Test scenario: AcornTorrent seeds, other client downloads.
    """
    print(f"{Fore.BLUE}\n=== UPLOAD SCENARIO ==={Style.RESET_ALL}")
    print(f"{Fore.BLUE}Tracker: {tracker}, Client: {client}{Style.RESET_ALL}")

    # Use pre-generated test data
    test_file = Path("/test-data/seed-file.dat")
    torrent_file = Path(f"/test-data/torrents/test-{tracker}.torrent")

    # Verify test files exist
    if not test_file.exists():
        print(f"{Fore.RED}Test file not found: {test_file}{Style.RESET_ALL}")
        print(f"{Fore.RED}Please run generate_test_data.py before running tests{Style.RESET_ALL}")
        return False

    if not torrent_file.exists():
        print(f"{Fore.RED}Torrent file not found: {torrent_file}{Style.RESET_ALL}")
        print(f"{Fore.RED}Please run generate_test_data.py before running tests{Style.RESET_ALL}")
        return False

    print(f"{Fore.BLUE}Using test file: {test_file}{Style.RESET_ALL}")
    print(f"{Fore.BLUE}Using torrent file: {torrent_file}{Style.RESET_ALL}")

    # 3. Start AcornTorrent as seeder
    print(f"{Fore.BLUE}Starting AcornTorrent as seeder...{Style.RESET_ALL}")
    # TODO: Invoke AcornTorrent to seed the file

    # 4. Start download with other client
    print(f"{Fore.BLUE}Starting {client} download...{Style.RESET_ALL}")
    # TODO: Add client-specific download commands

    # 5. Verify download
    print(f"{Fore.BLUE}Verifying download...{Style.RESET_ALL}")
    # TODO: Check that file was downloaded by other client and hash matches

    print(f"{Fore.YELLOW}Upload test NOT IMPLEMENTED YET{Style.RESET_ALL}")
    return False


def test_announce_scenario(tracker: str, client: str, tracker_url: str) -> bool:
    """
    Test scenario: Test tracker communication (announce/scrape).
    """
    print(f"{Fore.BLUE}\n=== ANNOUNCE SCENARIO ==={Style.RESET_ALL}")
    print(f"{Fore.BLUE}Tracker: {tracker}, Client: {client}{Style.RESET_ALL}")

    # Use pre-generated test data
    test_file = Path("/test-data/seed-file.dat")
    torrent_file = Path(f"/test-data/torrents/test-{tracker}.torrent")

    # Verify test files exist
    if not test_file.exists():
        print(f"{Fore.RED}Test file not found: {test_file}{Style.RESET_ALL}")
        print(f"{Fore.RED}Please run generate_test_data.py before running tests{Style.RESET_ALL}")
        return False

    if not torrent_file.exists():
        print(f"{Fore.RED}Torrent file not found: {torrent_file}{Style.RESET_ALL}")
        print(f"{Fore.RED}Please run generate_test_data.py before running tests{Style.RESET_ALL}")
        return False

    print(f"{Fore.BLUE}Using test file: {test_file}{Style.RESET_ALL}")
    print(f"{Fore.BLUE}Using torrent file: {torrent_file}{Style.RESET_ALL}")

    # 2. Test AcornTorrent announce to tracker
    print(f"{Fore.BLUE}Testing AcornTorrent announce to tracker...{Style.RESET_ALL}")

    # Run acorntorrent announce command
    result = subprocess.run(
        [
            "/bin-shared/acorntorrent",
            "announce",
            "--torrent",
            str(torrent_file),
            "--port",
            "6881",
            "--event",
            "started",
            "--verbose",
        ],
        capture_output=True,
        text=True,
        timeout=10,
    )

    print(f"{Fore.BLUE}AcornTorrent output:\n{result.stdout}{Style.RESET_ALL}")

    if result.stderr:
        print(f"{Fore.YELLOW}AcornTorrent stderr:\n{result.stderr}{Style.RESET_ALL}")

    # 3. Verify tracker response
    print(f"{Fore.BLUE}Verifying tracker response...{Style.RESET_ALL}")

    if result.returncode == 0 and "Successfully announced to tracker" in result.stdout:
        print(f"{Fore.GREEN}Announce test completed successfully!{Style.RESET_ALL}")
        return True
    else:
        print(f"{Fore.RED}Announce failed with exit code {result.returncode}{Style.RESET_ALL}")
        return False


def main() -> None:
    parser = argparse.ArgumentParser(description="Run integration test")
    parser.add_argument("--tracker", required=True, help="Tracker name")
    parser.add_argument("--client", required=True, help="Client name")
    parser.add_argument(
        "--scenario",
        required=True,
        choices=["download", "upload", "announce"],
        help="Test scenario",
    )

    args = parser.parse_args()

    tracker_url = os.environ.get("TRACKER_URL", f"http://{args.tracker}:6969/announce")

    print(f"{Fore.BLUE}\n{'=' * 60}{Style.RESET_ALL}")
    print(f"{Fore.BLUE}Integration Test{Style.RESET_ALL}")
    print(f"{Fore.BLUE}Tracker: {args.tracker}{Style.RESET_ALL}")
    print(f"{Fore.BLUE}Client: {args.client}{Style.RESET_ALL}")
    print(f"{Fore.BLUE}Scenario: {args.scenario}{Style.RESET_ALL}")
    print(f"{Fore.BLUE}{'=' * 60}\n{Style.RESET_ALL}")

    # Wait for tracker to be ready
    if not wait_for_tracker(tracker_url):
        print(f"{Fore.RED}Test failed: Tracker not ready{Style.RESET_ALL}")
        sys.exit(1)

    # Run appropriate test scenario
    success = False
    if args.scenario == "download":
        success = test_download_scenario(args.tracker, args.client, tracker_url)
    elif args.scenario == "upload":
        success = test_upload_scenario(args.tracker, args.client, tracker_url)
    elif args.scenario == "announce":
        success = test_announce_scenario(args.tracker, args.client, tracker_url)

    if success:
        print(f"{Fore.GREEN}\n{'=' * 60}{Style.RESET_ALL}")
        print(f"{Fore.GREEN}TEST PASSED{Style.RESET_ALL}")
        print(f"{Fore.GREEN}{'=' * 60}\n{Style.RESET_ALL}")
        sys.exit(0)
    else:
        print(f"{Fore.RED}\n{'=' * 60}{Style.RESET_ALL}")
        print(f"{Fore.RED}TEST FAILED{Style.RESET_ALL}")
        print(f"{Fore.RED}{'=' * 60}\n{Style.RESET_ALL}")
        sys.exit(1)


if __name__ == "__main__":
    main()
