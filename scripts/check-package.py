#!/usr/bin/env python3

from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]
VERSION = "0.3.0"


def require(condition: bool, message: str) -> None:
    if not condition:
        raise SystemExit(message)


def text(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def main() -> None:
    require(f'version = "{VERSION}"' in text("Cargo.toml"), "Cargo version mismatch")
    require(f'version = "{VERSION}"' in text("tools/centaur_context/pyproject.toml"), "tool version mismatch")
    require(f'version = "{VERSION}"' in text("compatibility.toml"), "compatibility version mismatch")
    require("[centaur_context]" in text("compatibility.toml"), "compatibility product key mismatch")
    require(
        f'TOOL_VERSION: &str = "{VERSION}"' in text("src/version.rs"),
        "API tool version mismatch",
    )
    require(f'image: centaur-context:{VERSION}' in text("deploy/deployment.yaml"), "deployment version mismatch")
    dockerfile = text("Dockerfile")
    require(dockerfile.count("@sha256:") == 3, "all three container bases must be digest-pinned")
    require("centaur-infra-env" not in text("deploy/deployment.yaml"), "deployment depends on a Centaur-core Secret")
    require("0.0.0.0/0" not in text("deploy/network-policy.yaml"), "default NetworkPolicy allows public egress")
    require(
        text("deploy/network-policy.yaml").count("apiVersion: networking.k8s.io/v1") == 1,
        "default package must not install policies selecting Centaur-owned pods",
    )
    require("Read and update" not in text("tools/centaur_context/pyproject.toml"), "read-only tool description regressed")
    migrations = [int(path.name.split("_", 1)[0]) for path in (ROOT / "migrations").glob("*.sql")]
    schema_version_match = re.search(
        r"^database_schema_version\s*=\s*(\d+)\s*$",
        text("compatibility.toml"),
        re.MULTILINE,
    )
    require(schema_version_match is not None, "compatibility schema version is missing")
    declared_schema_version = int(schema_version_match.group(1))
    require(
        max(migrations) == declared_schema_version,
        "database schema version does not match migrations",
    )
    api_schema_version_match = re.search(
        r"^pub const DATABASE_SCHEMA_VERSION: i64 = (\d+);$",
        text("src/version.rs"),
        re.MULTILINE,
    )
    require(api_schema_version_match is not None, "API schema version is missing")
    require(
        int(api_schema_version_match.group(1)) == declared_schema_version,
        "API schema version does not match compatibility metadata",
    )

    tracked = subprocess.check_output(["git", "ls-files"], cwd=ROOT, text=True).splitlines()
    local_path = re.compile(r"/Users/|[A-Za-z]:\\\\Users\\\\")
    legacy_name = re.compile(
        r"Centaur OS|centaur-os|centaur_os|CENTAUR_OS|centaur_tool_centaur_os",
        re.IGNORECASE,
    )
    legacy_allowlist = {
        "docs/setup.md",
        "scripts/backup.sh",
        "scripts/bootstrap-database.sh",
        "scripts/common.sh",
        "scripts/drop-database.sh",
        "scripts/install-kubernetes.sh",
        "scripts/restore.sh",
        "scripts/test_rename_contract.py",
        "scripts/uninstall-kubernetes.sh",
        "scripts/validate-backup-metadata.py",
        "src/db.rs",
        "src/schema.rs",
        "tests/database_contract.rs",
        "tests/artifact_embeddings.rs",
        "tests/intake_contract.rs",
        "tests/slack_runs.rs",
        "tests/source_intake_contract.rs",
        "tools/centaur_context/client.py",
        "tools/centaur_context/pyproject.toml",
        "tools/centaur_context/test_client.py",
        "web/vite.config.ts",
    }
    violations = []
    legacy_violations = []
    for relative in tracked:
        if relative == "scripts/check-package.py":
            continue
        path = ROOT / relative
        if path.is_file():
            try:
                content = path.read_text(encoding="utf-8")
            except UnicodeDecodeError:
                continue
            if local_path.search(content):
                violations.append(relative)
            if (
                legacy_name.search(content)
                and relative not in legacy_allowlist
            ):
                legacy_violations.append(relative)
    require(not violations, f"local machine paths found in: {', '.join(violations)}")
    require(
        not legacy_violations,
        f"unallowlisted legacy product names found in: {', '.join(legacy_violations)}",
    )

    manifests = [
        "deploy/deployment.yaml",
        "deploy/service.yaml",
        "deploy/network-policy.yaml",
        "deploy/secret.example.yaml",
        "deploy/provider-egress.example.yaml",
    ]
    for manifest in manifests:
        subprocess.run(
            ["kubectl", "apply", "--dry-run=server", "-f", str(ROOT / manifest)],
            check=True,
            stdout=subprocess.DEVNULL,
        )
    scripts = sorted((ROOT / "scripts").glob("*.sh"))
    for script in scripts:
        subprocess.run(["bash", "-n", str(script)], check=True)
    print("Package contract checks passed.")


if __name__ == "__main__":
    main()
