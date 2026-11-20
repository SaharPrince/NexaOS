# NCE-002l: Windows system PE corpus sample

## Purpose

Add a real Windows-distributed PE32+ AMD64 executable to the image-layout review without committing or redistributing the operating-system binary.

## Fixture and test

The Windows CI runner supplies `%WINDIR%\System32\where.exe` through `NEXA_WINDOWS_SYSTEM_PE_FIXTURE`. The integration test reads the file, runs the read-only inspector, and asks the pure layout planner to validate the image. It asserts the format and machine type and confirms inspection remains `NOT_ATTEMPTED`.

The binary is supplied by the CI operating system and is never executed by the test. No proprietary executable is added to the repository.

## Validation

The Windows test passed on commit `dcbc2bc`:

- Fixture: `where.exe`, 65,536 bytes.
- Parsed sections: 7.
- Planned copy bytes: 50,416.
- Planned zero-fill bytes: 0.
- Inspection and image-layout planning both succeeded; execution was not attempted.

This is one Windows system sample, not broad coverage of all Windows versions or application vendors.
