# Security Policy

NexaOS is experimental software. Do not use it to run untrusted executables.

## Reporting

Please do not publish details of a suspected vulnerability in an issue. Contact
the repository owner privately through GitHub's security reporting feature.

## NCE-001 safety boundaries

- The PE inspector reads bytes only; it never loads or executes the inspected file.
- All file offsets and ranges must be bounds-checked before reading.
- Malformed input must return an error, not panic.
- Parser changes should include regression tests for malformed inputs.
- Fuzzing is encouraged before accepting parser changes.
