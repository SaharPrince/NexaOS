# Security Policy

NexaOS is experimental software. Do not use it to run untrusted executables.

## Reporting

Please do not publish details of a suspected vulnerability in a public issue.
If GitHub private vulnerability reporting is enabled for this repository, use its
private reporting flow. If it is unavailable, ask the repository owner for a
private reporting channel before sharing technical details.

## Current safety boundaries

- The PE inspector reads bytes only; it never loads or executes the inspected file.
- The image-preparation pipeline produces non-executable data and does not create
  application processes, load Windows DLLs, or transfer control to PE entry points.
- The desktop's current Windows application path is preview-only. It does not
  execute selected files or create a Windows application process.
- The file browser is home-confined. Its limited folder-creation operation does
  not modify existing files or accept arbitrary paths.
- Do not treat NexaOS as a secure general-purpose Windows application runtime.
  Real application execution remains disabled pending separate security review.

## NCE-001 safety boundaries

- All file offsets and ranges must be bounds-checked before reading.
- Malformed input must return an error, not panic.
- Parser changes should include regression tests for malformed inputs.
- Fuzzing is encouraged before accepting parser changes.
- GitHub Actions runs Rust tests on Linux and Windows, Clippy with warnings denied, bounded parser fuzzing, and a scheduled Rust dependency advisory audit.
- Dependabot proposes weekly updates for Cargo dependencies and GitHub Actions. Review and validate every update before merging.

## NCE-007 security and privacy gate

The [NCE-007 milestone](docs/milestones/NCE-007.md) defines the required
application-isolation, credential-protection, privacy, IPC, update-integrity,
and failure-containment design before any Windows executable may be launched.

The intended policy is least privilege and privacy by default:
- No ambient access to browser cookies, saved passwords, MFA material, OS
  keyrings, SSH keys, or unrelated game sessions.
- No automatic privilege elevation or unsandboxed fallback.
- No telemetry by default; optional diagnostics require informed opt-in and
  must be minimized, bounded, and redacted.
- Sensitive capabilities such as clipboard and screen access require clear,
  scoped user intent where supported.
- Anti-cheat compatibility is limited to vendor- and publisher-approved paths.
  NexaOS will not bypass or tamper with anti-cheat or other security controls.

These are design requirements, not claims that the current preview implements
a secure application sandbox or can guarantee protection from third-party
account enforcement.
