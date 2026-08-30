import json
import os
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def run_common(script: str, env: dict[str, str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["bash", "-c", f'source "$1/scripts/common.sh"; {script}', "bash", str(ROOT)],
        env=env,
        text=True,
        capture_output=True,
        check=False,
    )


def test_legacy_environment_value_is_promoted() -> None:
    env = {"PATH": "/usr/bin:/bin", "CENTAUR_OS_SAMPLE": "legacy-value"}
    result = run_common(
        'resolve_legacy_env CENTAUR_CONTEXT_SAMPLE CENTAUR_OS_SAMPLE; printf %s "$CENTAUR_CONTEXT_SAMPLE"',
        env,
    )
    assert result.returncode == 0
    assert result.stdout == "legacy-value"


def test_conflicting_environment_values_fail_closed() -> None:
    env = {
        "PATH": "/usr/bin:/bin",
        "CENTAUR_CONTEXT_SAMPLE": "canonical",
        "CENTAUR_OS_SAMPLE": "legacy",
    }
    result = run_common(
        "resolve_legacy_env CENTAUR_CONTEXT_SAMPLE CENTAUR_OS_SAMPLE", env
    )
    assert result.returncode != 0
    assert "conflicts with legacy" in result.stderr


def test_backup_metadata_accepts_canonical_and_legacy_products(tmp_path: Path) -> None:
    validator = ROOT / "scripts/validate-backup-metadata.py"
    for product, database in [
        ("centaur-context", "centaur_context"),
        ("centaur-os", "centaur_os"),
    ]:
        metadata = tmp_path / f"{product}.json"
        metadata.write_text(
            json.dumps(
                {
                    "product": product,
                    "product_version": "0.2.0" if product == "centaur-context" else "0.1.0",
                    "database": database,
                    "schema_version": 11,
                    "format": "pg_dump-custom",
                }
            ),
            encoding="utf-8",
        )
        subprocess.run([sys.executable, str(validator), str(metadata)], check=True)


def test_backup_metadata_rejects_unrelated_product(tmp_path: Path) -> None:
    metadata = tmp_path / "invalid.json"
    metadata.write_text(
        json.dumps(
            {
                "product": "other-product",
                "product_version": "1.0.0",
                "database": "centaur_context",
                "schema_version": 10,
                "format": "pg_dump-custom",
            }
        ),
        encoding="utf-8",
    )
    result = subprocess.run(
        [sys.executable, str(ROOT / "scripts/validate-backup-metadata.py"), str(metadata)],
        text=True,
        capture_output=True,
        check=False,
    )
    assert result.returncode != 0
    assert "unsupported product discriminator" in result.stderr


def test_backup_metadata_rejects_future_schema(tmp_path: Path) -> None:
    validator = ROOT / "scripts/validate-backup-metadata.py"
    metadata = tmp_path / "backup.json"
    metadata.write_text(
        json.dumps(
            {
                "product": "centaur-context",
                "product_version": "0.2.0",
                "database": "centaur_context",
                "schema_version": 12,
                "format": "pg_dump-custom",
            }
        ),
        encoding="utf-8",
    )
    result = subprocess.run(
        [sys.executable, str(validator), str(metadata)],
        check=False,
        capture_output=True,
        text=True,
    )
    assert result.returncode != 0
    assert "unsupported schema version" in result.stderr


def fake_kubectl(tmp_path: Path) -> Path:
    executable = tmp_path / "kubectl"
    executable.write_text(
        """#!/usr/bin/env bash
if [[ "$*" == "config current-context" ]]; then printf test-context; exit 0; fi
if [[ "$*" == *"get deployment centaur-os --ignore-not-found --output=name"* ]]; then
  printf deployment.apps/centaur-os; exit 0
fi
if [[ "$*" == *"get deployment centaur-os --output=jsonpath={.spec.replicas}"* ]]; then
  printf %s "${LEGACY_REPLICAS:-1}"; exit 0
fi
if [[ "$*" == *"get deployment centaur-os --output=jsonpath={.status.readyReplicas}"* ]]; then
  printf %s "${LEGACY_READY:-1}"; exit 0
fi
if [[ "$*" == *"get secret centaur-context-env"* ]]; then printf dmFsdWU=; exit 0; fi
exit 0
""",
        encoding="utf-8",
    )
    executable.chmod(0o755)
    return executable


def run_installer(tmp_path: Path, *extra: str, replicas: str = "1", ready: str = "1"):
    fake_kubectl(tmp_path)
    env = os.environ.copy()
    env.update(
        {
            "PATH": f"{tmp_path}:{env['PATH']}",
            "CENTAUR_CONTEXT_KUBE_CONTEXT": "test-context",
            "CENTAUR_CONTEXT_NAMESPACE": "test-namespace",
            "LEGACY_REPLICAS": replicas,
            "LEGACY_READY": ready,
        }
    )
    return subprocess.run(
        [
            "bash",
            str(ROOT / "scripts/install-kubernetes.sh"),
            "--image",
            "centaur-context:0.2.0",
            "--apply",
            *extra,
        ],
        env=env,
        text=True,
        capture_output=True,
        check=False,
    )


def test_installer_refuses_parallel_legacy_deployment(tmp_path: Path) -> None:
    result = run_installer(tmp_path)
    assert result.returncode != 0
    assert "refuse parallel install" in result.stderr


def test_installer_requires_legacy_deployment_to_be_scaled_down(tmp_path: Path) -> None:
    result = run_installer(tmp_path, "--legacy-cutover")
    assert result.returncode != 0
    assert "fully scaled to zero" in result.stderr


def test_installer_allows_explicit_scaled_down_handoff(tmp_path: Path) -> None:
    result = run_installer(
        tmp_path, "--legacy-cutover", replicas="0", ready="0"
    )
    assert result.returncode == 0, result.stderr
