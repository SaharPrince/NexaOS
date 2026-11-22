# NCE-007A: Threat Model and Data-Flow Inventory

## Status and scope

**Design artifact for review. This document does not implement or prove an operating-system security boundary.**

NCE-007A turns the security goals in [NCE-007](NCE-007.md) into a reviewable inventory of protected assets, trust boundaries, attacker capabilities, expected data flows, assumptions, and residual risks. It is a prerequisite for later isolation and broker implementation work.

The current NexaOS preview remains non-executing for Windows PE files. Nothing in this document authorizes a loader, executable mapping, DLL loading, process launch, privilege change, anti-cheat integration, or production deployment.

## 1. Security objectives

NexaOS must preserve all five product requirements together:

1. Broad practical Windows application and game compatibility, using only legitimate provider- and publisher-approved support paths.
2. High performance and low resource use, including modest hardware.
3. A complete, customizable, accessible desktop.
4. Strong isolation, system integrity, resilience, and recoverability.
5. Protection of personal files, credentials, browser sessions, gaming sessions, and private activity.

The security design must not solve performance problems by silently removing useful features, and must not solve compatibility problems by weakening isolation or account protection.

### Security outcomes

- An application receives only the capabilities explicitly granted to its verified runtime identity.
- One application cannot read or alter another application's private state by default.
- Applications cannot obtain ambient access to host credentials, browser sessions, keyrings, or unrelated personal data.
- User grants are narrow, understandable, inspectable, revocable, and enforced before protected access.
- Malformed input, failed policy setup, broker failure, or unavailable mandatory controls never cause an unsandboxed fallback.
- Security enforcement remains available offline and does not depend on optional AI, reputation services, or network access.
- Diagnostic and security-event data are minimized and do not become a secondary source of sensitive information.
- Updates and recovery preserve the integrity of the system and its trust roots.

## 2. System and trust assumptions

### In-scope system

The design covers the NexaOS host and desktop, the future Windows compatibility runtime, per-application state, the permission/broker path, host-resource grants, credentials and account material, diagnostics, update/recovery, and supported anti-cheat integrations.

### Assumptions requiring validation

- The supported Linux kernel and its security features are known and versioned for each target release.
- The kernel, boot chain, and verified NexaOS security components are not already compromised.
- The host user session and trusted permission UI have not already been taken over by an attacker with equivalent privileges.
- Cryptographic algorithms and implementations are selected from maintained, reviewed libraries.
- Package and update provenance can be verified before privileged installation.
- Hardware and firmware security properties vary; unsupported or unverifiable configurations are reported honestly.
- Third-party applications, publishers, launchers, websites, downloaded content, plugins, and compatibility dependencies are not trusted merely because they are popular or signed.

These are assumptions, not guarantees. Each release must document which assumptions were tested and which remain environmental dependencies.

## 3. Protected assets and security properties

| Asset | Examples | Required properties |
|---|---|---|
| Host integrity | Kernel, boot configuration, system files, package manager, security policy | Integrity, authenticated changes, recoverability |
| User data | Documents, source code, photos, downloads, removable media | Confidentiality, integrity, explicit access |
| Credentials | Password stores, tokens, MFA recovery, keyrings, SSH/API keys | Confidentiality, controlled use, no diagnostic leakage |
| Browser sessions | Cookies, session tokens, browser profiles | Isolation from compatibility applications by default |
| Gaming sessions | Launcher tokens, account sessions, protected game state | Isolation and publisher-approved use only |
| Application state | Per-app prefix, virtual registry, caches, imported dependencies | Per-app confidentiality and integrity |
| User interaction | Clipboard, screen, microphone, camera, input, accessibility APIs | Explicit, scoped, revocable authorization |
| Network identity | DNS activity, IP configuration, account/device identifiers | Privacy, minimum disclosure, user-visible network policy |
| Permission state | Grants, denials, expiry, revocation, application identity | Integrity, provenance, auditable decisions |
| Update trust | Signing keys, metadata, payload hashes, recovery state | Authenticity, anti-rollback, availability |
| Security diagnostics | Local event summaries, crash metadata, opt-in reports | Minimization, redaction, bounded retention |

### Priority classification

- **Critical:** boot/update trust, host integrity, credential stores, permission policy, broker authority, and isolation configuration.
- **High:** personal files, browser/game sessions, per-app private state, clipboard and screen access.
- **Moderate:** non-sensitive application metadata and minimized diagnostic summaries.
- **Availability-sensitive:** desktop session, networking, update/recovery, and compatibility runtime responsiveness.

The classification is a design aid. It does not mean lower-priority data may be collected or exposed without authorization.

## 4. Attacker model

### In-scope attacker capabilities

The design assumes an attacker may control one or more of the following:

- A Windows application running inside its assigned compatibility environment.
- A malicious installer, plugin, script, DLL, or application dependency.
- A malformed or adversarial PE file targeting inspection, preparation, protocol parsing, or future loading code.
- A website, document, archive, filename, URL, redirect, or downloaded payload presented to an application.
- A local unprivileged process attempting to impersonate another app, reuse stale authority, access a broker, or cross an application boundary.
- A compromised application update or substituted dependency.
- A hostile remote server communicating with an application.
- A malicious or compromised package mirror or update transport.
- A legitimate application behaving unexpectedly after an update or compromise.

Attackers may race process creation and exit, exploit malformed inputs, supply deceptive permission descriptions, replay stale requests, or deliberately create resource pressure.

### Out-of-scope or residual attacker capabilities

This milestone does not claim to withstand:

- A fully compromised kernel, boot chain, firmware, or hardware trust root.
- An attacker with unrestricted physical access and the ability to modify or replace the device.
- A malicious host administrator who already controls the running system.
- Vulnerabilities in third-party applications or online services that are outside NexaOS's control.
- Account enforcement decisions made by publishers, game operators, or other third parties.
- Unknown hardware or driver defects.

These risks must be documented, not hidden behind claims of perfect security.

## 5. Trust boundaries

| Boundary | Untrusted side | Trusted responsibility | Required failure behavior |
|---|---|---|---|
| PE input to inspection | File bytes and metadata | Bounded, data-only parser | Reject malformed input; never execute it |
| Desktop to permission UI | Application-supplied request and description | Trusted OS presentation and policy lookup | Do not trust app-supplied identity or explanatory text |
| Desktop to broker | Local client and serialized request | Authenticate client identity; validate schema and authority | Reject spoofed, stale, malformed, or oversized requests |
| Broker to policy enforcement | Requested capability and resource | Deterministic policy decision before access | Deny when policy or identity is uncertain |
| Runtime to host filesystem | Application-controlled paths and handles | Per-app confinement and narrow grants | Deny escape, traversal, and unauthorized access |
| Runtime to another app | App-controlled IPC and shared resources | Process and data isolation | No ambient cross-app access |
| Runtime to credentials | Requests for secrets or session state | Protected stores and explicit hand-off | Deny ambient access; never copy secrets implicitly |
| Runtime to devices and interaction | Device, clipboard, screen, audio, camera, input requests | OS-mediated scoped permissions | Deny absent a valid grant |
| Runtime to network | Application traffic and local socket requests | User-visible network policy and host boundary | No undisclosed privileged proxy or policy bypass |
| Updater to host | Metadata, payloads, mirrors, package artifacts | Signature, provenance, version and rollback validation | Reject unverifiable updates; preserve recovery |
| Diagnostics to storage or network | Runtime-controlled strings and crash metadata | Redaction, minimization, consent and retention | No secret leakage; external reporting off by default |
| Anti-cheat integration | Game/provider requirements and protected components | Official provider/publisher-approved contract | Mark unsupported when no approved path exists |

A UI prompt is not itself an enforcement boundary. Enforcement must occur at the point where the protected resource is accessed.

## 6. Data-flow inventory

The following flows define what information may cross boundaries. Each flow requires a concrete implementation design and test evidence before runtime enablement.

### DF-01: Application discovery and PE inspection

**Inputs:** selected file bytes, file metadata, user-selected path, parser results.

**Processing:** bounded, non-executing inspection and structured metadata extraction.

**Allowed outputs:** file format facts, architecture, sections/import metadata where safely parsed, and validation findings.

**Forbidden flows:** execution, shell interpolation, arbitrary host file traversal, credential access, and automatic trust decisions based only on a signature or hash.

**Evidence:** malformed-file fuzzing, bounded reads, path validation, and proof that inspection never starts a process.

### DF-02: Application identity and launch request

**Inputs:** selected application identity, requested runtime profile, user action, and structured launch parameters.

**Processing:** trusted identity resolution, policy lookup, schema validation, and authorization checks.

**Allowed outputs:** a bounded request to the runtime broker containing only approved fields.

**Forbidden flows:** shell command construction from user input, app-controlled publisher identity, ambient environment inheritance, or automatic administrator elevation.

**Evidence:** identity spoofing tests, malformed request tests, environment canaries, and explicit proof that the current preview remains non-executing.

### DF-03: Permission request and decision

**Inputs:** requesting app identity, capability, resource identifier, access mode, duration, and justification metadata.

**Processing:** trusted UI displays the verified requester and exact requested scope; deterministic policy checks whether the grant is allowed.

**Allowed outputs:** a narrowly scoped authorization decision and a bounded event summary.

**Forbidden flows:** blanket grants inferred from installation, download, click, legal consent, publisher reputation, or one unrelated permission.

**Evidence:** overbroad grant rejection, deceptive-description tests, expiry/revocation tests, and proof that the access path checks authorization before disclosure.

### DF-04: Host filesystem access

**Inputs:** application file operations, user-selected resources, file handles, and path components.

**Processing:** per-app filesystem confinement and policy-enforced user grants.

**Allowed outputs:** access to the app's private state and only the exact host resources authorized for the requested operation.

**Forbidden flows:** ambient home-directory access, path traversal, symlink escape, descriptor leakage, and implicit access to other app prefixes.

**Evidence:** adversarial path/symlink tests, descriptor hygiene tests, cross-app isolation tests, and revocation tests.

### DF-05: Credential and session boundary

**Inputs:** attempted access to password stores, browser/game profiles, MFA material, SSH/API keys, keyrings, or authentication hand-off.

**Processing:** deny ambient access; require a separately reviewed, user-mediated, narrow integration if one is ever supported.

**Allowed outputs:** only the minimum authorized result for the explicit operation; never a wholesale copy of a profile or credential database.

**Forbidden flows:** secrets in arguments, environment variables, app prefixes, logs, crash dumps, telemetry, or automatic session import.

**Evidence:** synthetic secret-canary tests across process launch, filesystem access, logs, crash output, and diagnostics.

### DF-06: Clipboard, screen, audio, camera, and input

**Inputs:** application requests for interaction or device capabilities.

**Processing:** OS-controlled permission UI and platform enforcement where supported.

**Allowed outputs:** only the authorized operation and scope.

**Forbidden flows:** silent capture, background monitoring without an appropriate grant, and use of one permission to imply another.

**Evidence:** positive and negative tests for each supported device/capability, revocation, and behavior when the platform cannot enforce a requested scope.

### DF-07: Network access

**Inputs:** application-originated network requests and local socket operations.

**Processing:** host network stack and any explicitly enabled per-app network policy.

**Allowed outputs:** traffic allowed by the documented network policy.

**Forbidden flows:** undisclosed NexaOS proxying, hidden analytics, privilege-bearing local socket exposure, or reputation lookups on mandatory access-control paths.

**Evidence:** offline operation tests, local socket boundary tests, and documented behavior when optional reputation data is unavailable.

### DF-08: Security events and diagnostics

**Inputs:** minimized event type, app identity, decision result, resource category, and bounded error metadata.

**Processing:** local bounded event handling, redaction, retention policy, and optional user-initiated export.

**Allowed outputs:** local explanations and opt-in reports containing only reviewed diagnostic fields.

**Forbidden flows:** file contents, passwords, tokens, clipboard content, keystrokes, full sensitive command lines, and routine cross-app activity tracking.

**Evidence:** canary scans of logs/crash artifacts, retention tests, opt-in/off-by-default tests, and export review.

### DF-09: Update and recovery

**Inputs:** signed update metadata, version, payload, provenance, and recovery state.

**Processing:** trust-chain verification, version/rollback checks, integrity verification, and recoverable installation.

**Allowed outputs:** an authenticated installation or a clear rejection/recovery result.

**Forbidden flows:** unauthenticated update execution, silent trust-root replacement, downgrade, or partial installation that leaves policy disabled.

**Evidence:** altered signature/payload, rollback, interrupted update, low-storage, and recovery tests.

### DF-10: Anti-cheat and publisher integration

**Inputs:** documented provider requirements, game version, supported runtime configuration, and approval evidence.

**Processing:** compatibility review and testing only under official provider/publisher-approved arrangements.

**Allowed outputs:** a per-title compatibility status with exact tested versions and limitations.

**Forbidden flows:** bypass, spoofing, tampering, disabling security checks, or claims based solely on a game reaching its menu.

**Evidence:** documented authorization, reproducible tests, and explicit unsupported status when approval is absent.

## 7. Cross-cutting security requirements

### Identity and authority

- Bind every authorization decision to a stable app identity, exact capability, resource scope, access mode, and applicable lifetime.
- Do not treat a process ID alone as a durable identity; process reuse and launch races must be considered.
- Applications cannot issue or expand their own grants.
- Revocation must invalidate subsequent access and broker-held authority where technically possible.
- Ambiguous identity, missing policy, or broker failure denies the protected operation.

### Data minimization

- Collect only the fields needed for the requested function.
- Keep process and security event data local by default.
- Do not log secrets or private content to explain a denial.
- Bound event queues, logs, crash artifacts, and retention.
- Make optional diagnostics explicit, opt-in, and revocable.

### Availability and performance

- Mandatory access decisions must be local, deterministic, and bounded.
- Event-driven analysis is preferred over continuous polling and full-disk scans.
- Optional reputation or ML analysis cannot be on a protected operation's authorization critical path.
- Under resource pressure, defer optional analysis; do not disable isolation, credential boundaries, or permission enforcement.
- Measure the same workloads with and without protection on documented hardware. Report latency distributions and resource overhead, not a single best-case number.

### Transparency and user control

- Explain what resource is requested, by which app, for which operation, and whether the decision is temporary or persistent.
- Show the current grant and provide a clear revocation route.
- Do not use alarming language or claim an application is malicious solely because it requests a permission.
- Distinguish a policy denial from a confirmed malware verdict.
- Public documentation must distinguish implemented controls from design requirements.

## 8. Residual risks and dependencies

| Risk or dependency | Why it remains | Required treatment |
|---|---|---|
| Kernel or boot-chain compromise | Host enforcement depends on the platform trust base | Supported-kernel policy, security updates, integrity checks, recovery |
| Firmware and hardware defects | NexaOS cannot fully control vendor firmware | Hardware support matrix and explicit limitations |
| Host account takeover | A hostile process with equivalent user-session authority may interfere with user workflows | Session hardening, privilege separation, clear residual-risk statement |
| Third-party application vulnerabilities | Apps may be compromised before or during use | Per-app isolation, least privilege, patching, user-visible provenance |
| Publisher/anti-cheat policy | Approval is controlled externally | Official support process; report unsupported titles honestly |
| Unknown parser/protocol defects | Complex input surfaces cannot be proven bug-free | Memory-safe implementation where practical, fuzzing, bounds, review |
| User-approved malicious action | A user can still authorize a harmful but technically permitted operation | Exact scope, clear prompts, reversible grants where possible |
| Availability attacks | A malicious app can consume resources within its allowed limits | Resource controls, bounded queues, fair scheduling, graceful containment |
| Supply-chain compromise | Dependencies and update infrastructure are external inputs | Provenance, signed artifacts, SBOM, review, incident response |

Residual risks must be revisited when the kernel baseline, desktop session, runtime architecture, update design, or supported hardware changes.

## 9. Required implementation evidence

NCE-007A is accepted only when reviewers can trace each critical asset to a boundary, a data flow, a required control, and a test.

Minimum evidence package:

1. A maintained asset and trust-boundary inventory.
2. A data-flow diagram or equivalent reviewed table covering all DF-01 through DF-10 flows.
3. A documented attacker-capability and assumption review.
4. A residual-risk register with owner or responsible workstream and mitigation status.
5. Traceability from each high/critical asset to NCE-007B through NCE-007H.
6. A test plan using synthetic fixtures and disposable identities only.
7. A public-claim review confirming that no proposed control is described as implemented.
8. Review of the exact commit and passing CI for documentation validation and any introduced tooling.

### Exit criteria

- Every asset has a confidentiality, integrity, availability, or privacy requirement.
- Every trust boundary has a named untrusted side, enforcement responsibility, and fail-closed behavior.
- Every sensitive data flow identifies inputs, processing, permitted outputs, forbidden flows, and evidence.
- Attacker assumptions and out-of-scope risks are explicit.
- No data flow relies on a language model, online reputation service, or network connection for mandatory enforcement.
- No real PE execution is enabled by completing this document.
- NCE-007B, NCE-007C, and later stages remain separately reviewed and gated.

## 10. Review checklist

- [ ] Asset inventory covers host, user data, credentials, sessions, interaction, network, permissions, updates, and diagnostics.
- [ ] Threat actors and attacker capabilities are realistic and bounded.
- [ ] All trust boundaries identify enforcement points rather than relying on UI prompts.
- [ ] All sensitive data flows have permitted and forbidden outputs.
- [ ] Credential and browser/game session isolation is explicit.
- [ ] Offline operation and failure behavior are explicit.
- [ ] Performance requirements preserve mandatory security enforcement.
- [ ] Residual risks and external dependencies are documented.
- [ ] Evidence and exit criteria are testable.
- [ ] Reviewers confirm the document does not authorize executable runtime work.

## Related documents

- [NCE-007 security and privacy design](NCE-007.md)
- [Secure transparency policy](../SECURE-TRANSPARENCY.md)
- [Product and engineering roadmap](../ROADMAP.md)
