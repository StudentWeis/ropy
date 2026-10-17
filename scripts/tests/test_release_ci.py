"""A release must use the latest main CI run for its exact source commit."""

import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
SHA = "a" * 40


class ReleaseCiTests(unittest.TestCase):
    def run_gate(self, runs, api_failure=False):
        with tempfile.TemporaryDirectory() as directory:
            work = Path(directory)
            response = work / "response.json"
            response.write_text(json.dumps({"workflow_runs": runs}))
            gh = work / "gh"
            gh.write_text('#!/bin/sh\n[ "$FAIL_API" = 0 ] || exit 1\ncat "$RESPONSE"\n')
            gh.chmod(0o755)
            return subprocess.run(
                ["python3", str(ROOT / "scripts/check/check_release_ci.py"),
                 "StudentWeis/ropy", SHA], capture_output=True, text=True,
                env=dict(os.environ, PATH=f"{work}:{os.environ['PATH']}",
                         RESPONSE=str(response), FAIL_API=str(int(api_failure))),
            )

    def run_record(self):
        return dict(id=1, head_sha=SHA, head_branch="main", event="push",
                    status="completed", conclusion="success")

    def test_release_requires_matching_successful_main_run(self):
        self.assertEqual(self.run_gate([self.run_record()]).returncode, 0)
        for changes in ({"head_sha": "b" * 40}, {"head_branch": "feature"},
                        {"event": "pull_request"}, {"status": "in_progress"},
                        {"conclusion": "failure"}, {"conclusion": "cancelled"},
                        {"conclusion": "skipped"}):
            with self.subTest(changes=changes):
                record = self.run_record() | changes
                self.assertNotEqual(self.run_gate([record]).returncode, 0)

    def test_release_rejects_missing_run_or_api_failure(self):
        self.assertNotEqual(self.run_gate([]).returncode, 0)
        self.assertNotEqual(self.run_gate([self.run_record()], api_failure=True).returncode, 0)

    def test_release_does_not_fall_back_to_older_success(self):
        old = self.run_record()
        new = old | {"id": 2, "status": "in_progress", "conclusion": None}
        self.assertNotEqual(self.run_gate([old, new]).returncode, 0)
