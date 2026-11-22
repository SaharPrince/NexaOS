# Secure Transparency Policy

## Purpose

NexaOS should remain secure even when attackers know its technology stack and
understand its published security architecture. Hiding Linux, Rust, dependencies,
or the existence of a security boundary is not a substitute for enforcing that
boundary correctly.

## Public by default

The project may publicly document:

- The Linux and Rust engineering foundations and major third-party components.
- High-level architecture, trust boundaries, threat models, and security goals.
- Permission semantics, isolation requirements, privacy commitments, and known
  limitations.
- Dependency manifests and lockfiles, build instructions, test strategies, and
  security advisories.
- Reproducible validation results, including failures and unsupported scenarios.

Security design should be reviewable. Public documentation must clearly separate
implemented controls from proposals, prototypes, and future acceptance criteria.

## Never commit or publish

Never place the following in source control, issues, logs, release notes, or
public artifacts:

- Passwords, access tokens, session cookies, API credentials, or recovery codes.
- Private signing keys, update credentials, or other secret key material.
- Production secrets, private infrastructure credentials, or authenticated
  internal service URLs.
- Personal data or diagnostic records that are not necessary and explicitly
  approved for publication.
- Unpatched vulnerability details that would materially enable exploitation
  before a coordinated fix is available.

If a credential is exposed, treat it as compromised: revoke or rotate it and
investigate its use. Removing it from the latest commit does not undo exposure
from repository history or external copies.

## Narrow security-review exceptions

Before publishing operational details, review whether they reveal a concrete,
currently exploitable weakness. Examples include non-public production
configuration, privileged internal interfaces whose disclosure creates a
specific attack path, or sensitive detection thresholds that can be directly
used to evade a defense.

Any restriction should be narrow, documented, and tied to a real risk. Do not
withhold ordinary architecture, design rationale, or implementation details
merely because they describe how NexaOS works.

## Required pre-publication review

Before merging or publishing code, documentation, workflows, or release assets,
review for:

1. Secrets, credentials, private keys, and accidental personal data.
2. Unpatched vulnerabilities or exploit-enabling reproduction details.
3. Privileged interfaces or operational configuration that expose a concrete
   attack path.
4. Unnecessary disclosure of private infrastructure.
5. Claims that overstate implementation, security, privacy, compatibility, or
   performance evidence.

Use automated secret scanning where it is available and maintain dependency
inventories. A clean scan is not proof that a repository contains no secrets;
human review and incident response remain necessary.

## Vulnerability reporting

Use GitHub private vulnerability reporting if it is enabled for the repository.
Otherwise, ask the repository owner for a private reporting channel. Do not
publish suspected vulnerability details in a public issue before coordinated
disclosure.

## Dependency and release integrity

Track dependencies through committed manifests and lockfiles, review automated
dependency updates, and keep security-audit workflows least-privileged. Release
artifacts must be traceable to a reviewed commit and accompanied by integrity
information where applicable. Never commit signing secrets or publish private
update credentials.

## Status discipline

A documented security requirement is not an implemented defense. A passing CI
job is not proof of a secure operating system. Public claims must identify what
was actually built, what was tested, on which configuration, and what remains
unverified.
