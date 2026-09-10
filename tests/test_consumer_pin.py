import json
from pathlib import Path
import subprocess
import tempfile
import tomllib
import unittest


ROOT = Path(__file__).resolve().parents[1]
URL = "https://github.com/lyra-os-linux/lyra-vega-dbus"
VERSION = tomllib.loads((ROOT / "Cargo.toml").read_text())["package"]["version"]
REVISION = "a" * 40


class ConsumerPinTests(unittest.TestCase):
    def check(self, reference, source):
        with tempfile.TemporaryDirectory() as directory:
            consumer = Path(directory)
            (consumer / "vega-gtk").mkdir()
            fields = {"git": URL, **reference}
            declaration = ", ".join(f"{key} = {json.dumps(value)}" for key, value in fields.items())
            (consumer / "vega-gtk/Cargo.toml").write_text("[dependencies]\nlyra-vega-dbus = { " + declaration + " }\n")
            (consumer / "Cargo.lock").write_text(
                f'[[package]]\nname = "lyra-vega-dbus"\nversion = "{VERSION}"\nsource = {json.dumps(source)}\n')
            result = subprocess.run(["bash", str(ROOT / "scripts/check-consumer-pin.sh"),
                                     str(consumer), "vega"], capture_output=True, text=True)
            return result.returncode

    def test_accepts_matching_release_tag_and_full_revision(self):
        self.assertEqual(self.check({"tag": "v" + VERSION}, f"git+{URL}?tag=v{VERSION}#{REVISION}"), 0)
        self.assertEqual(self.check({"rev": REVISION}, f"git+{URL}?rev={REVISION}#{REVISION}"), 0)

    def test_rejects_floating_mismatched_or_incomplete_references(self):
        for reference, source in (
            ({"branch": "main"}, f"git+{URL}?branch=main#{REVISION}"),
            ({"rev": REVISION[:7]}, f"git+{URL}?rev={REVISION[:7]}#{REVISION}"),
            ({"rev": REVISION}, f"git+{URL}?rev={REVISION}#{'b' * 40}"),
            ({"rev": REVISION}, f"git+{URL}?tag=v{VERSION}#{REVISION}"),
            ({"tag": "v0.0.0"}, f"git+{URL}?tag=v0.0.0#{REVISION}"),
            ({"tag": "v" + VERSION}, f"git+{URL}?tag=v{VERSION}#short"),
            ({"tag": "v" + VERSION, "rev": REVISION}, f"git+{URL}?rev={REVISION}#{REVISION}"),
        ):
            with self.subTest(reference=reference, source=source):
                self.assertNotEqual(self.check(reference, source), 0)
