#!/usr/bin/env python3
"""Download and verify the pinned NexaOS real-world PE test corpus.

Only extracts named PE files as data. No downloaded executable is launched.
"""
from __future__ import annotations

import hashlib
import io
from pathlib import Path
import sys
import urllib.request
import zipfile

ROOT = Path("target/real-world-pe")
MAX_DOWNLOAD = 100 * 1024 * 1024
MAX_MEMBER = 100 * 1024 * 1024

SAMPLES = (
    {
        "name": "ripgrep.exe",
        "url": "https://github.com/BurntSushi/ripgrep/releases/download/15.2.0/ripgrep-15.2.0-x86_64-pc-windows-msvc.zip",
        "archive_sha256": "71b2fef860abe467217a538ff31de02f5258807c0129f771846f87bd029aafc5",
        "member": "rg.exe",
        "sha256": "14231169855EC5205CF5A1B6F1DB358FF4AED4247C86B69CE8AAE647C77F6680",
    },
    {
        "name": "bat.exe",
        "url": "https://github.com/sharkdp/bat/releases/download/v0.26.1/bat-v0.26.1-x86_64-pc-windows-msvc.zip",
        "archive_sha256": "0f729b4b6f5f28d395c641eacc2e9ff68d0096b85aa0eec344aa62425144b69b",
        "member": "bat.exe",
        "sha256": "9EA9F7AB1C27AA5C9390E91543F9F47C6BD17BC53F3E30AC9DB762F28F55C4FD",
    },
    {
        "name": "jq.exe",
        "url": "https://github.com/jqlang/jq/releases/download/jq-1.8.2/jq-windows-amd64.exe",
        "sha256": "A6FC67FEDAF9128A3309A1E2EBB8B986AECCF70122EE46D2CB4849E423F0C627",
    },
    {
        "name": "sqlite3.dll",
        "url": "https://www.sqlite.org/2026/sqlite-dll-win-x64-3540100.zip",
        "archive_sha3_256": "c2d09c8babb9399277055c057c97a53f1ffd7c5f11d0c1f2118ffdfa39f2daa4",
        "member": "sqlite3.dll",
        "sha256": "4B59D3CDED9DC1608544160F3B4B10AC974C6AC0D6FFC23064E5F4C8DBD05AEA",
    },
)


def digest(data: bytes, algorithm: str) -> str:
    return hashlib.new(algorithm, data).hexdigest()


def download(url: str) -> bytes:
    request = urllib.request.Request(
        url, headers={"User-Agent": "NexaOS-PE-corpus-validation/1.0"}
    )
    with urllib.request.urlopen(request, timeout=45) as response:
        data = response.read(MAX_DOWNLOAD + 1)
    if len(data) > MAX_DOWNLOAD:
        raise ValueError(f"download exceeds {MAX_DOWNLOAD} bytes: {url}")
    return data


def extract_exact_member(archive: bytes, wanted_name: str) -> bytes:
    with zipfile.ZipFile(io.BytesIO(archive)) as zipped:
        matches = [
            info for info in zipped.infolist()
            if not info.is_dir() and Path(info.filename).name.lower() == wanted_name.lower()
        ]
        if len(matches) != 1:
            raise ValueError(
                f"expected exactly one {wanted_name} member, found {len(matches)}"
            )
        info = matches[0]
        if info.file_size > MAX_MEMBER:
            raise ValueError(f"archive member too large: {info.filename}")
        if info.file_size > max(info.compress_size, 1) * 200:
            raise ValueError(f"archive member compression ratio is excessive: {info.filename}")
        return zipped.read(info)


def main() -> None:
    ROOT.mkdir(parents=True, exist_ok=True)
    manifest = []
    for sample in SAMPLES:
        print(f"Downloading {sample['name']} from its pinned upstream release...")
        payload = download(sample["url"])

        if "archive_sha256" in sample:
            actual = digest(payload, "sha256")
            if actual.lower() != sample["archive_sha256"].lower():
                raise ValueError(f"{sample['name']}: archive SHA-256 mismatch: {actual}")
            payload = extract_exact_member(payload, sample["member"])
        elif "archive_sha3_256" in sample:
            actual = digest(payload, "sha3_256")
            if actual.lower() != sample["archive_sha3_256"].lower():
                raise ValueError(f"{sample['name']}: archive SHA3-256 mismatch: {actual}")
            payload = extract_exact_member(payload, sample["member"])

        actual_binary = digest(payload, "sha256")
        if actual_binary.lower() != sample["sha256"].lower():
            raise ValueError(f"{sample['name']}: binary SHA-256 mismatch: {actual_binary}")

        destination = ROOT / sample["name"]
        destination.write_bytes(payload)
        manifest.append(
            f"{sample['name']} {len(payload)} bytes SHA256 {actual_binary.upper()}"
        )
        print(f"Verified {sample['name']}: {len(payload)} bytes SHA256 {actual_binary.upper()}")

    (ROOT / "SHA256SUMS.txt").write_text("\n".join(manifest) + "\n", encoding="utf-8")
    print(f"Verified {len(SAMPLES)} files. PE files were not executed.")


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(f"PE corpus download/verification failed: {error}", file=sys.stderr)
        raise SystemExit(1)
