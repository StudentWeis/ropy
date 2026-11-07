"""Icon checks distinguish source-relative includes from asset identifiers."""

import importlib.util
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


SCRIPT = Path(__file__).resolve().parents[1] / "check/check_icons.py"
SPEC = importlib.util.spec_from_file_location("check_icons", SCRIPT)
icons = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(icons)


class IconTests(unittest.TestCase):
    def test_icons_resolve_includes_relative_to_each_rust_file(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "assets/icons").mkdir(parents=True)
            (root / "src/nested").mkdir(parents=True)
            for name in ("embedded.svg", "bytes.png", "runtime.svg", "method.svg"):
                (root / "assets/icons" / name).write_text("icon")
            (root / "src/nested/view.rs").write_text('''
include_str!("../../assets/icons/embedded.svg");
include_bytes!("../../assets/icons/bytes.png");
Assets::get("icons/runtime.svg");
Icon::empty().path("icons/method.svg");
include_str!("fixture.svg");
''')
            # Source-local images are valid includes, not bundled asset identifiers.
            (root / "src/nested/fixture.svg").write_text("fixture")
            result = subprocess.run([sys.executable, str(SCRIPT), "--root", str(root)],
                                    capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            (root / "src/nested/view.rs").write_text('include_bytes!("../../assets/icons/missing.png");')
            missing = icons.check_missing_icons(icons.collect_asset_icons(root / "assets"),
                                                icons.collect_referenced_icons(root / "src"))
            self.assertEqual(missing, ["icons/missing.png"])


if __name__ == "__main__":
    unittest.main()
