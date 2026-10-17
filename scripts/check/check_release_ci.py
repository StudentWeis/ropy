#!/usr/bin/env python3
"""Require successful main CI for the exact commit being released."""

import argparse
import json
import subprocess
import sys
from urllib.parse import urlencode


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("repository")
    parser.add_argument("revision")
    args = parser.parse_args()
    query = urlencode(dict(head_sha=args.revision, branch="main", event="push", per_page=100))
    result = subprocess.run(
        ["gh", "api", f"repos/{args.repository}/actions/workflows/ci.yml/runs?{query}"],
        capture_output=True, text=True,
    )
    if result.returncode:
        sys.stderr.write(result.stderr)
        return result.returncode
    runs = [run for run in json.loads(result.stdout)["workflow_runs"]
            if run["head_sha"] == args.revision and run["head_branch"] == "main"
            and run["event"] == "push"]
    latest = max(runs, key=lambda run: run["id"], default=None)
    if latest is None or latest["status"] != "completed" or latest["conclusion"] != "success":
        print("Release blocked: the latest main CI run for this exact commit must succeed. "
              "Merge the release preparation PR, wait for main CI, then retry the release.",
              file=sys.stderr)
        return 1
    print(f"Release CI verified: {args.revision}, run {latest['id']}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
