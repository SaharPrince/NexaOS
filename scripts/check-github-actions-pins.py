#!/usr/bin/env python3
"""Require immutable GitHub Action references and non-persistent checkout tokens."""

from __future__ import annotations

import re
import sys
from pathlib import Path

WORKFLOW_DIR = Path(".github/workflows")
ACTION_LINE = re.compile(r"\buses:\s*([^\s#]+)")
FULL_COMMIT = re.compile(r"[0-9a-f]{40}\Z")
CHECKOUT_ACTION = "actions/checkout"


def step_block(lines: list[str], start: int) -> list[str]:
    """Return the current step block, ending at the next sibling step."""
    indent = len(lines[start]) - len(lines[start].lstrip())
    end = len(lines)
    for index in range(start + 1, len(lines)):
        line = lines[index]
        stripped = line.lstrip()
        if not stripped or stripped.startswith("#"):
            continue
        current_indent = len(line) - len(stripped)
        if current_indent <= indent and stripped.startswith("- "):
            end = index
            break
    return lines[start:end]


def validate_workflow(path: Path) -> list[str]:
    lines = path.read_text(encoding="utf-8").splitlines()
    errors: list[str] = []
    action_count = 0

    for index, line in enumerate(lines):
        match = ACTION_LINE.search(line)
        if not match:
            continue

        reference = match.group(1)
        if reference.startswith("./"):
            continue

        action_count += 1
        if "@" not in reference:
            errors.append(f"{path}:{index + 1}: external action has no immutable ref: {reference}")
            continue

        action, revision = reference.rsplit("@", 1)
        if not FULL_COMMIT.fullmatch(revision):
            errors.append(
                f"{path}:{index + 1}: action must use a full 40-character commit SHA: {reference}"
            )
            continue

        if action.lower() == CHECKOUT_ACTION:
            block = step_block(lines, index)
            if not any(
                re.match(r"^\s* persist-credentials:\s*false\s*(?:#.*)?$", item)
                for item in block
            ):
                errors.append(
                    f"{path}:{index + 1}: checkout must set persist-credentials: false"
                )

    if action_count == 0:
        print(f"{path}: no external actions to validate")
    return errors


def main() -> int:
    workflows = sorted(
        path for path in WORKFLOW_DIR.iterdir()
        if path.is_file() and path.suffix in {".yml", ".yaml"}
    )
    if not workflows:
        print(f"No workflow files found in {WORKFLOW_DIR}", file=sys.stderr)
        return 1

    errors = [error for path in workflows for error in validate_workflow(path)]
    if errors:
        for error in errors:
            print(f"FAIL: {error}", file=sys.stderr)
        return 1

    print(
        f"GitHub Actions pin check: PASS "
        f"({len(workflows)} workflow files; all external actions pinned; "
        "checkout credentials not persisted)"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
