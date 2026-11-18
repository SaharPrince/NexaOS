#!/usr/bin/env python3
"""Check that the NCE-005a sample table and aggregate evidence agree."""

from __future__ import annotations

import re
import sys
from pathlib import Path

DOC = Path("docs/milestones/NCE-005a.md")
EXPECTED = {
    "ripgrep 15.2.0",
    "bat 0.26.1",
    "jq 1.8.2",
    "SQLite 3.54.1",
}
NUMBER = r"[0-9][0-9,]*"


def number(value: str) -> int:
    return int(value.replace(",", ""))


def fail(message: str) -> None:
    print(f"NCE-005a evidence check: FAIL: {message}", file=sys.stderr)
    raise SystemExit(1)


def main() -> None:
    text = DOC.read_text(encoding="utf-8")
    lines = text.splitlines()

    if "four individually published upstream Windows PE binaries" not in text.lower():
        fail("status text does not describe the four-sample corpus")
    if "Four individually published upstream Windows AMD64 PE binaries" not in text:
        fail("real-world sample introduction does not describe all four samples")

    rows: dict[str, str] = {}
    for line in lines:
        if not line.startswith("|"):
            continue
        cells = [cell.strip() for cell in line.strip().strip("|").split("|")]
        if cells and cells[0] in EXPECTED:
            if cells[0] in rows:
                fail(f"duplicate sample row: {cells[0]}")
            rows[cells[0]] = line

    if set(rows) != EXPECTED:
        missing = sorted(EXPECTED - set(rows))
        extra = sorted(set(rows) - EXPECTED)
        fail(f"sample table mismatch; missing={missing}, unexpected={extra}")

    totals = {"entries": 0, "absolute": 0, "dir64": 0, "patches": 0}
    entry_pattern = re.compile(
        rf"({NUMBER}) entries (({NUMBER}) ABSOLUTE, ({NUMBER}) DIR64)"
    )
    patch_pattern = re.compile(rf"({NUMBER}) DIR64 patches planned and applied")

    for sample, row in rows.items():
        entry_match = entry_pattern.search(row)
        patch_match = patch_pattern.search(row)
        if not entry_match or not patch_match:
            fail(f"could not parse relocation evidence for {sample}")

        entries, absolute, dir64 = map(number, entry_match.groups())
        patches = number(patch_match.group(1))
        if entries != absolute + dir64:
            fail(f"{sample}: entry count does not equal ABSOLUTE + DIR64")
        if patches != dir64:
            fail(f"{sample}: applied patch count does not equal DIR64 count")

        totals["entries"] += entries
        totals["absolute"] += absolute
        totals["dir64"] += dir64
        totals["patches"] += patches

    aggregate = re.search(
        rf"Across the four samples: \\*\\*({NUMBER}) relocation entries\\*\\*, "
        rf"comprising \\*\\*({NUMBER}) ABSOLUTE\\*\\* entries and "
        rf"\\*\\*({NUMBER}) DIR64\\*\\* entries; all "
        rf"\\*\\*({NUMBER}) DIR64 patches\\*\\* were planned and applied successfully\\.",
        text,
    )
    if not aggregate:
        fail("could not parse the four-sample aggregate sentence")

    documented = dict(zip(
        ("entries", "absolute", "dir64", "patches"),
        map(number, aggregate.groups()),
        strict=True,
    ))
    if totals != documented:
        fail(f"sample totals {totals} do not match documented aggregate {documented}")

    if "Execution was **not attempted**." not in text:
        fail("execution-safety statement is missing")

    print(
        "NCE-005a evidence check: PASS "
        f"({len(rows)} samples; {totals['entries']:,} entries; "
        f"{totals['absolute']:,} ABSOLUTE; {totals['dir64']:,} DIR64 patches)"
    )


if __name__ == "__main__":
    main()
