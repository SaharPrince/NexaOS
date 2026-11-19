# NexaOS desktop-shell model

This crate is the first UI-independent desktop/file-manager integration slice.
It provides an in-memory directory view model, safe display text handling,
structured .exe launch requests, and a deterministic mock launcher.

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

This is a model layer, not yet a graphical shell, file manager, compatibility
runtime, or executable launcher. A future GUI should bind to these models and
keep the same privilege and execution boundary.
