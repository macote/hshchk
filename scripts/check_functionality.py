#!/usr/bin/env python3
"""Check a reviewed acceptance inventory against tests available on this platform.

Run only after both Rust and E2E suites pass. Discovery is traceability evidence,
not proof that an assertion is adequate; that remains a review responsibility.
"""
import argparse
import json
from pathlib import Path
import platform


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rust-tests", type=Path, required=True)
    parser.add_argument("--e2e-tests", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--inventory", type=Path,
                        default=Path(__file__).resolve().parent.parent / "docs/functionality-inventory.json")
    args = parser.parse_args()
    inventory = json.loads(args.inventory.read_text(encoding="utf-8"))
    rust = {line.removesuffix(": test").split("::")[-1] for line in
            args.rust_tests.read_text(encoding="utf-8").splitlines() if line.endswith(": test")}
    e2e = set(args.e2e_tests.read_text(encoding="utf-8").splitlines())
    available = rust | e2e
    host = platform.system()
    rows = []
    ids = set()
    for requirement in inventory["requirements"]:
        if requirement["id"] in ids:
            raise ValueError("duplicate requirement ID")
        ids.add(requirement["id"])
        if host not in requirement["platforms"]:
            continue
        names = requirement["tests"]
        covered = bool(names) and all(name in available for name in names)
        rows.append({"id": requirement["id"], "behavior": requirement["behavior"],
                     "critical": requirement["critical"], "mapped": covered,
                     "missing_tests": [name for name in names if name not in available]})
    if not rows:
        raise ValueError("no requirements apply to this platform")
    mapped = sum(row["mapped"] for row in rows)
    percent = mapped * 100 / len(rows)
    critical_gaps = [row["id"] for row in rows if row["critical"] and not row["mapped"]]
    result = {"platform": host, "applicable": len(rows), "mapped": mapped,
              "mapped_percent": round(percent, 2), "critical_gaps": critical_gaps,
              "requirements": rows}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(f"{host}: {mapped}/{len(rows)} requirements mapped ({percent:.2f}%); critical gaps: {critical_gaps}")
    for row in rows:
        if not row["mapped"]:
            print(f"Unmapped: {row['id']} {row['behavior']}")
    if percent < 80 or critical_gaps:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
