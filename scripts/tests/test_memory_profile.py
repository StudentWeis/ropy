"""Exercise profiling without macOS tools or a running application."""

import os
from pathlib import Path
import subprocess
import tempfile
import unittest


SCRIPT = Path(__file__).resolve().parents[1] / "memory_profile.sh"


class MemoryProfileTests(unittest.TestCase):
    def run_profile(self, failure=""):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name)
        tools = root / "bin"
        tools.mkdir()
        fixtures = {
            "pgrep": "echo 123",
            "ps": "echo process",
            "vmmap": '''if [ "$FAIL_TOOL" = "vmmap:$1" ]; then
echo "vmmap failed" >&2; exit 23
fi
echo memory''',
            "heap": '''if [ "$FAIL_TOOL" = "heap:$1" ]; then
echo "heap failed" >&2; exit 23
fi
i=0
while [ "$i" -lt 2000 ]; do echo "allocation $i"; i=$((i + 1)); done''',
        }
        for name, body in fixtures.items():
            executable = tools / name
            executable.write_text("#!/bin/bash\n" + body + "\n")
            executable.chmod(0o755)
        result = subprocess.run(
            ["bash", str(SCRIPT)], cwd=root,
            env={**os.environ, "PATH": f"{tools}:{os.environ['PATH']}", "FAIL_TOOL": failure},
            capture_output=True, text=True, check=False,
        )
        reports = list((root / "target").glob("memory_profile_*"))
        return result, reports[0]

    def test_tool_failures_are_reported_without_success_message(self):
        for failure in ("vmmap:--summary", "vmmap:123", "heap:123", "heap:--addresses=all"):
            with self.subTest(failure=failure):
                result, _ = self.run_profile(failure)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("failed", result.stderr)
                self.assertNotIn("=== Full reports saved", result.stdout)

    def test_large_heap_output_is_saved_and_summarized(self):
        result, reports = self.run_profile()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("=== Full reports saved", result.stdout)
        full = (reports / "heap.txt").read_text().splitlines()
        self.assertEqual(len(full), 2000)
        self.assertEqual((reports / "heap_summary.txt").read_text().splitlines(), full[:50])
        self.assertEqual((reports / "heap_addresses.txt").read_text().splitlines(), full[-20:])
