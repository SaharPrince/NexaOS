# NexaOS desktop-shell model

This crate is the first UI-independent desktop/file-manager integration slice.
It provides an in-memory directory view model, safe display text handling,
structured .exe launch requests, a UI-facing controller, and a deterministic
mock launcher.

## UI integration boundary

A graphical frontend can use `DesktopController` to:
- read the current directory model and selected entry;
- select or clear an entry without accessing the filesystem itself;
- activate a selected `.exe` candidate through one structured method;
- receive a `LaunchRecord` with a request ID and explicit preview-only status;
- cancel a preview and read the resulting request records.

The controller is an adapter boundary, not a graphical frontend or a real
file-manager backend. Directory contents are still supplied by a caller.
Activation creates only a mock preview record.

## Safety boundary

- The mock launcher never creates a process or executes a file.
- It does not invoke a shell, load DLLs, map executable memory, call PE entry
  points, or connect to PreparedImage.
- Launch requests require an absolute path and an .exe extension. Paths remain
  structured PathBuf values; they are never concatenated into command text.
- A PreviewOnly response explicitly says no process was created.
- Cancellation only changes mock request state; it does not signal a process.
- File names are untrusted display text; control characters are replaced.
- Directory contents are supplied by a caller; this crate does not traverse the
  filesystem or grant access to files.
- No default .exe file association or automatic execution is introduced.

This is a model/controller layer, not yet a graphical shell, filesystem-backed
file manager, compatibility runtime, or executable launcher. A future GUI
should bind to this controller and keep the same privilege and execution
boundary.

## Versioned browser snapshot boundary

The controller now accepts a bounded JSON snapshot through
`DesktopController::from_browser_snapshot_json`. Protocol version 1 carries
an absolute trusted home-root path, a relative current-directory path, and up
to 1,000 direct-child entries. Entry kinds are limited to `directory` and
`file`; paths are relative components rather than shell fragments or
caller-supplied commands.

The decoder rejects unknown JSON fields, unsupported protocol versions,
absolute or traversing relative paths, nested entry paths, mismatched names,
duplicate entries, and oversized snapshots. It performs no filesystem access.
The resulting model still routes `.exe` activation only to the preview-only
mock launcher.

The Qt file browser now sends this versioned request to the fixed installed
`/usr/lib/nexaos/nexa-desktop-controller` helper using a structured argument
list and JSON on standard input. The helper is built from this crate and is
invoked with a short timeout; no filename or request data is interpolated into
a shell command. The helper returns a structured preview-only response. It
does not open the selected file or start the selected application.

The helper is a one-shot process boundary, not a long-running privileged
service. Requests are capped at 1 MiB, and the controller validates the
snapshot before handling the selected relative path. The graphical frontend
fails closed if the helper is missing, times out, or returns an invalid/error
response. The helper itself runs with the logged-in user's privileges.

This is a real preview-controller connection, **not an executable launcher**.
No file association or `.exe` execution is enabled. A future real runtime still
requires its own reviewed process model, sandbox, crash isolation, and
vendor-approved anti-cheat gates.
