"""Small runtime comparison of the decode hot path against the official upstream wheel (C++ core)
at the release this package tracks. Both run the same script in separate interpreters with the same
Python, torch, numpy, transformers, and pydantic; only `xgrammar` differs.
"""

import json
import shutil
import subprocess
import sys
import sysconfig
from importlib import metadata
from pathlib import Path

import pytest

SCRIPT = Path(__file__).with_name("upstream_perf.py")
PACKAGE_ROOT = Path(__file__).resolve().parents[1]
# Shared CI runners are noisy; a larger slowdown than this is a real regression.
MAX_RATIO = 1.25
SHARED_PACKAGES = ("torch", "numpy", "transformers", "pydantic")

pytestmark = [
    pytest.mark.skipif(sys.version_info < (3, 9), reason="upstream publishes no Python 3.8 wheel"),
    pytest.mark.skipif(
        bool(sysconfig.get_config_var("Py_GIL_DISABLED")), reason="upstream has no free-threaded wheel"
    ),
    pytest.mark.skipif(shutil.which("uv") is None, reason="uv creates the upstream environment"),
]


def _run(command):
    completed = subprocess.run(command, check=True, capture_output=True, text=True, timeout=1800)
    return json.loads(completed.stdout.strip().splitlines()[-1])


def test_decode_hot_path_is_not_slower_than_upstream(capsys):
    upstream = ["uv", "run", "--isolated", "--no-project", "--python", sys.executable]
    upstream += ["--with", f"xgrammar=={metadata.version('xgrammar')}"]
    for name in SHARED_PACKAGES:
        upstream += ["--with", f"{name}=={metadata.version(name)}"]
    commands = {"upstream": upstream + ["python", str(SCRIPT)], "ours": [sys.executable, str(SCRIPT)]}

    runs = {"upstream": [], "ours": []}
    for label in ("upstream", "ours", "ours", "upstream"):  # balanced order against drift
        runs[label].append(_run(commands[label]))
    assert Path(runs["ours"][0]["xgrammar"]).resolve().is_relative_to(PACKAGE_ROOT)
    assert not Path(runs["upstream"][0]["xgrammar"]).resolve().is_relative_to(PACKAGE_ROOT)

    rows = []
    for workload, operations in runs["ours"][0]["results"].items():
        for operation in operations:
            ours = min(run["results"][workload][operation]["p50_ns"] for run in runs["ours"])
            upstream_ns = min(run["results"][workload][operation]["p50_ns"] for run in runs["upstream"])
            rows.append((workload, operation, ours, upstream_ns, ours / upstream_ns))
    with capsys.disabled():
        print(f"\n{'workload':<14} {'operation':<24} {'ours p50':>10} {'upstream p50':>13} {'ratio':>7}")
        for workload, operation, ours, upstream_ns, ratio in rows:
            print(f"{workload:<14} {operation:<24} {ours:>8}ns {upstream_ns:>11}ns {ratio:>7.2f}")
    slower = [row for row in rows if row[4] > MAX_RATIO]
    assert not slower, f"slower than upstream by more than {MAX_RATIO}x: {slower}"
