from itertools import product
from pathlib import Path
from typing import Any

import yaml

# Configuration
TRACKERS = ["chihaya"]
CLIENTS = ["transmission", "rtorrent", "qbittorrent"]
SCENARIOS = ["announce", "download", "upload"]

OUTPUT_DIR = Path(__file__).parent / "compose-files"
DOCKERFILE_DIR = Path(__file__).parent.parent / "dockerfiles"
PROJECT_ROOT = Path(__file__).parent.parent


def get_client_config(client_name) -> dict[str, Any]:
    configs = {
        "transmission": {
            "build": {
                "context": str(PROJECT_ROOT),
                "dockerfile": str(
                    DOCKERFILE_DIR / "clients" / "dockerfile.transmission"
                ),
            },
            "init": True,
            "volumes": [
                "./test-data/downloads-{client}:/data/downloads",
                "./test-data/watch-{client}:/data/watch",
            ],
            "networks": ["torrent-test"],
            "depends_on": ["{tracker}"],
        },
        "rtorrent": {
            "build": {
                "context": str(PROJECT_ROOT),
                "dockerfile": str(DOCKERFILE_DIR / "clients" / "dockerfile.rtorrent"),
            },
            "volumes": [
                "./test-data/downloads-{client}:/data/downloads",
                "./test-data/watch-{client}:/data/watch",
                "./test-data/session-{client}:/data/session",
            ],
            "networks": ["torrent-test"],
            "depends_on": ["{tracker}"],
        },
        "qbittorrent": {
            "build": {
                "context": str(PROJECT_ROOT),
                "dockerfile": str(
                    DOCKERFILE_DIR / "clients" / "dockerfile.qbittorrent"
                ),
            },
            "volumes": [
                "./test-data/downloads-{client}:/data/downloads",
            ],
            "networks": ["torrent-test"],
            "depends_on": ["{tracker}"],
        },
    }

    return configs[client_name]


def get_tracker_config(tracker_name) -> dict[str, Any]:
    configs = {
        "chihaya": {
            "build": {
                "context": str(PROJECT_ROOT),
                "dockerfile": str(DOCKERFILE_DIR / "trackers" / "dockerfile.chihaya"),
            },
            "ports": ["6969:6969", "6880:6880"],
            "networks": ["torrent-test"],
        },
    }

    return configs[tracker_name]


def get_acorntorrent_config() -> dict[str, Any]:
    return {
        "build": {
            "context": str(PROJECT_ROOT),
            "dockerfile": str(PROJECT_ROOT / "dockerfile"),
        },
        "volumes": [
            "./test-data/downloads-acorn:/data/downloads",
            "./test-data/torrents:/data/torrents",
            "./test-data/bin:/shared/bin",
        ],
        "networks": ["torrent-test"],
        "depends_on": ["{tracker}"],
        "command": "sh -c 'cp /app/acorntorrent /shared/bin/ && tail -f /dev/null'",
    }


def generate_compose_file(tracker, client, scenario) -> dict[str, Any]:
    compose = {
        "services": {},
        "networks": {"torrent-test": {"driver": "bridge"}},
    }

    # Add tracker service
    tracker_config = get_tracker_config(tracker)
    compose["services"][tracker] = tracker_config

    # Add client service
    client_config = get_client_config(client)
    # Replace placeholders
    client_config_str = (
        str(client_config).replace("{tracker}", tracker).replace("{client}", client)
    )
    client_config = eval(client_config_str)
    compose["services"][client] = client_config

    # Add AcornTorrent service
    acorn_config = get_acorntorrent_config()
    acorn_config_str = str(acorn_config).replace("{tracker}", tracker)
    acorn_config = eval(acorn_config_str)
    compose["services"]["acorntorrent"] = acorn_config

    # Add test orchestrator service
    compose["services"]["test-orchestrator"] = {
        "build": {
            "context": str(PROJECT_ROOT / "integration_tests"),
            "dockerfile": str(PROJECT_ROOT / "integration_tests" / "dockerfile.test-orchestrator"),
        },
        "volumes": [
            "../test-scripts:/test-scripts",
            "./test-data:/test-data",
            "./test-data/bin:/bin-shared:ro",
        ],
        "networks": ["torrent-test"],
        "depends_on": [tracker, client, "acorntorrent"],
        "working_dir": "/test-scripts",
        "command": f"python run_test.py --tracker={tracker} --client={client} --scenario={scenario}",
        "environment": {
            "TRACKER": tracker,
            "CLIENT": client,
            "SCENARIO": scenario,
            "TRACKER_URL": f"http://{tracker}:6969/announce",
        },
    }

    return compose


def main() -> None:
    # Create output directory
    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)

    # Generate compose file for each combination
    combinations = []
    for tracker, client, scenario in product(TRACKERS, CLIENTS, SCENARIOS):
        # Generate compose file
        compose = generate_compose_file(tracker, client, scenario)

        # Write to file
        filename = f"docker-compose-{tracker}-{client}-{scenario}.yml"
        filepath = OUTPUT_DIR / filename

        with filepath.open("w") as f:
            yaml.dump(compose, f, default_flow_style=False, sort_keys=False)

        combinations.append((tracker, client, scenario, filename))
        print(f"Generated: {filename}")

    # Generate master list
    with (OUTPUT_DIR / "test_matrix.txt").open("w") as f:
        f.write("# Integration Test Matrix\n")
        f.write(f"# Total combinations: {len(combinations)}\n\n")
        for tracker, client, scenario, filename in combinations:
            f.write(f"{tracker:15} {client:15} {scenario:10} -> {filename}\n")

    print(f"\nGenerated {len(combinations)} docker-compose files")
    print(f"Output directory: {OUTPUT_DIR}")


if __name__ == "__main__":
    main()
