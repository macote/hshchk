"""Guard the acceptance gate against critical omissions and denominator mistakes."""
import json
from pathlib import Path
import platform
import subprocess
import sys
import tempfile
import unittest


class FunctionalityGateTests(unittest.TestCase):
    def check(self, requirements, names):
        with tempfile.TemporaryDirectory(prefix="hshchk-gate-test-") as directory:
            root = Path(directory)
            inventory = root / "inventory.json"
            inventory.write_text(json.dumps({"requirements": requirements}))
            rust = root / "rust.txt"
            rust.write_text("".join(name + ": test\n" for name in names))
            e2e = root / "e2e.txt"
            e2e.write_text("")
            output = root / "report.json"
            result = subprocess.run([sys.executable, str(Path(__file__).with_name("check_functionality.py")),
                                     "--inventory", str(inventory), "--rust-tests", str(rust),
                                     "--e2e-tests", str(e2e), "--output", str(output)],
                                    capture_output=True, text=True, timeout=30)
            return result, json.loads(output.read_text()) if output.exists() else None

    def requirement(self, identifier, critical=False, platforms=None):
        return {"id": identifier, "behavior": identifier, "critical": critical,
                "platforms": platforms or [platform.system()], "tests": [identifier]}

    def test_missing_critical_requirement_fails_even_above_80_percent(self):
        requirements = [self.requirement(str(i)) for i in range(9)] + [self.requirement("critical", True)]
        result, report = self.check(requirements, [str(i) for i in range(9)])
        self.assertEqual(result.returncode, 1)
        self.assertEqual(report["mapped_percent"], 90)
        self.assertEqual(report["critical_gaps"], ["critical"])

    def test_inapplicable_requirements_do_not_change_this_platform_denominator(self):
        requirements = [self.requirement("here", True), self.requirement("elsewhere", True, ["OtherOS"])]
        result, report = self.check(requirements, ["here"])
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(report["applicable"], 1)
        self.assertEqual(report["mapped_percent"], 100)

    def test_duplicate_ids_are_rejected_instead_of_inflating_coverage(self):
        result, report = self.check([self.requirement("same"), self.requirement("same")], ["same"])
        self.assertNotEqual(result.returncode, 0)
        self.assertIsNone(report)


if __name__ == "__main__":
    unittest.main()
