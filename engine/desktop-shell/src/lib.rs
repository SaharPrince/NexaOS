//! Safe, UI-independent desktop interaction model for NexaOS.
//!
//! This crate models file-manager entries and the launch-request boundary.
//! Its mock launcher NEVER executes files, invokes a shell, starts processes,
//! loads PE images, or connects to the PE preparation APIs.

use std::{
    collections::BTreeMap,
    ffi::OsStr,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    Directory,
    File,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileEntry {
    path: PathBuf,
    display_name: String,
    kind: EntryKind,
}

impl FileEntry {
    pub fn new(path: PathBuf, display_name: impl Into<String>, kind: EntryKind) -> Self {
        Self {
            path,
            display_name: sanitize_display_text(&display_name.into()),
            kind,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    pub fn kind(&self) -> EntryKind {
        self.kind
    }

    pub fn is_windows_executable_candidate(&self) -> bool {
        self.kind == EntryKind::File
            && self
                .path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case(OsStr::new("exe")))
    }
}

/// Replace terminal/UI control characters before untrusted names reach a view.
pub fn sanitize_display_text(input: &str) -> String {
    input
        .chars()
        .map(|character| {
            if character.is_control() {
                '\u{fffd}'
            } else {
                character
            }
        })
        .collect()
}

/// Versioned, untrusted directory snapshot exchanged across the future UI boundary.
#[derive(Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrowserSnapshot {
    protocol_version: u32,
    root: PathBuf,
    current_directory: PathBuf,
    entries: Vec<BrowserSnapshotEntry>,
}

#[derive(Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct BrowserSnapshotEntry {
    name: String,
    relative_path: PathBuf,
    kind: SnapshotEntryKind,
    #[allow(dead_code)]
    size: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum SnapshotEntryKind {
    Directory,
    File,
}

/// Fail-closed errors for data received from the graphical file browser.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserSnapshotError {
    InvalidJson,
    UnsupportedProtocolVersion,
    RootMustBeAbsolute,
    RootContainsTraversal,
    CurrentDirectoryMustBeRelative,
    CurrentDirectoryContainsTraversal,
    TooManyEntries,
    InvalidEntryPath,
    EntryNameMismatch,
    DuplicateEntry,
    EntryOutsideCurrentDirectory,
}

impl BrowserSnapshot {
    /// Decode and validate a bounded JSON snapshot. This never reads the filesystem.
    pub fn from_json(input: &str) -> Result<Self, BrowserSnapshotError> {
        serde_json::from_str(input).map_err(|_| BrowserSnapshotError::InvalidJson)
    }

    /// Convert a validated snapshot into the existing UI-independent model.
    pub fn into_directory_model(self) -> Result<DirectoryModel, BrowserSnapshotError> {
        if self.protocol_version != 1 {
            return Err(BrowserSnapshotError::UnsupportedProtocolVersion);
        }
        if !self.root.is_absolute() {
            return Err(BrowserSnapshotError::RootMustBeAbsolute);
        }
        if self.root.components().any(|part| matches!(part, std::path::Component::ParentDir | std::path::Component::CurDir)) {
            return Err(BrowserSnapshotError::RootContainsTraversal);
        }
        if self.current_directory.is_absolute() {
            return Err(BrowserSnapshotError::CurrentDirectoryMustBeRelative);
        }
        if self.current_directory.components().any(|part| !matches!(part, std::path::Component::Normal(_))) {
            return Err(BrowserSnapshotError::CurrentDirectoryContainsTraversal);
        }
        if self.entries.len() > 1000 {
            return Err(BrowserSnapshotError::TooManyEntries);
        }

        let current = self.root.join(&self.current_directory);
        let mut seen = std::collections::BTreeSet::new();
        let mut entries = Vec::with_capacity(self.entries.len());
        for entry in self.entries {
            let mut components = entry.relative_path.components();
            let Some(std::path::Component::Normal(component)) = components.next() else {
                return Err(BrowserSnapshotError::InvalidEntryPath);
            };
            if components.next().is_some() {
                return Err(BrowserSnapshotError::EntryOutsideCurrentDirectory);
            }
            if component.to_str() != Some(entry.name.as_str()) || entry.name.is_empty() {
                return Err(BrowserSnapshotError::EntryNameMismatch);
            }
            if !seen.insert(entry.relative_path.clone()) {
                return Err(BrowserSnapshotError::DuplicateEntry);
            }
            let path = current.join(&entry.relative_path);
            let kind = match entry.kind {
                SnapshotEntryKind::Directory => EntryKind::Directory,
                SnapshotEntryKind::File => EntryKind::File,
            };
            entries.push(FileEntry::new(path, entry.name, kind));
        }
        Ok(DirectoryModel::new(current, entries))
    }
}

/// In-memory directory view model. It performs no filesystem traversal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectoryModel {
    current_directory: PathBuf,
    entries: Vec<FileEntry>,
    selected: Option<usize>,
}

impl DirectoryModel {
    pub fn new(current_directory: PathBuf, mut entries: Vec<FileEntry>) -> Self {
        entries.sort_by(|left, right| {
            let rank = |kind| match kind {
                EntryKind::Directory => 0,
                EntryKind::File => 1,
            };
            rank(left.kind).cmp(&rank(right.kind)).then_with(|| {
                left.display_name
                    .to_lowercase()
                    .cmp(&right.display_name.to_lowercase())
            })
        });
        Self {
            current_directory,
            entries,
            selected: None,
        }
    }

    pub fn current_directory(&self) -> &Path {
        &self.current_directory
    }

    pub fn entries(&self) -> &[FileEntry] {
        &self.entries
    }

    pub fn selected(&self) -> Option<&FileEntry> {
        self.selected.and_then(|index| self.entries.get(index))
    }

    pub fn select(&mut self, index: usize) -> Result<(), DirectoryError> {
        if index >= self.entries.len() {
            return Err(DirectoryError::SelectionOutOfRange);
        }
        self.selected = Some(index);
        Ok(())
    }

    pub fn clear_selection(&mut self) {
        self.selected = None;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectoryError {
    SelectionOutOfRange,
}

/// Structured launch data; this type never stores shell command text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchRequest {
    path: PathBuf,
}

impl LaunchRequest {
    pub fn new(path: PathBuf) -> Result<Self, LaunchError> {
        if path.as_os_str().is_empty() {
            return Err(LaunchError::EmptyPath);
        }
        if !path.is_absolute() {
            return Err(LaunchError::PathMustBeAbsolute);
        }
        if !path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case(OsStr::new("exe")))
        {
            return Err(LaunchError::UnsupportedFileType);
        }
        Ok(Self { path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchError {
    EmptyPath,
    PathMustBeAbsolute,
    UnsupportedFileType,
    UnknownRequest,
    RequestAlreadyCancelled,
    RequestIdExhausted,
    NoSelection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RequestId(u64);

impl RequestId {
    pub fn get(self) -> u64 {
        self.0
    }
}

/// No Started state exists: this mock never starts processes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchStatus {
    PreviewOnly,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchRecord {
    pub id: RequestId,
    pub path: PathBuf,
    pub status: LaunchStatus,
    pub explanation: &'static str,
}

/// Deterministic UI-development mock. It never executes files or creates processes.
#[derive(Debug)]
pub struct MockLauncher {
    next_id: u64,
    records: BTreeMap<RequestId, LaunchRecord>,
}

impl Default for MockLauncher {
    fn default() -> Self {
        Self::new()
    }
}

impl MockLauncher {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            records: BTreeMap::new(),
        }
    }

    pub fn submit(&mut self, request: LaunchRequest) -> Result<LaunchRecord, LaunchError> {
        let next_id = self
            .next_id
            .checked_add(1)
            .ok_or(LaunchError::RequestIdExhausted)?;
        let id = RequestId(self.next_id);
        self.next_id = next_id;
        let record = LaunchRecord {
            id,
            path: request.path,
            status: LaunchStatus::PreviewOnly,
            explanation: "Preview only: no process was created and no file was executed.",
        };
        self.records.insert(id, record.clone());
        Ok(record)
    }

    /// Cancellation updates only mock state; no process exists to interrupt.
    pub fn cancel(&mut self, id: RequestId) -> Result<LaunchRecord, LaunchError> {
        let record = self
            .records
            .get_mut(&id)
            .ok_or(LaunchError::UnknownRequest)?;
        if record.status == LaunchStatus::Cancelled {
            return Err(LaunchError::RequestAlreadyCancelled);
        }
        record.status = LaunchStatus::Cancelled;
        record.explanation = "Cancelled in the mock UI; no process was created.";
        Ok(record.clone())
    }

    pub fn get(&self, id: RequestId) -> Option<&LaunchRecord> {
        self.records.get(&id)
    }

    pub fn records(&self) -> impl Iterator<Item = &LaunchRecord> {
        self.records.values()
    }
}

/// UI-facing controller that connects a directory selection to the mock launcher.
///
/// A graphical frontend can use this narrow API without building shell command
/// strings or gaining permission to execute files. The controller remains
/// preview-only until a separately reviewed runtime exists.
#[derive(Debug)]
pub struct DesktopController {
    directory: DirectoryModel,
    launcher: MockLauncher,
}

impl DesktopController {
    /// Construct the preview-only controller from a validated UI snapshot.
    ///
    /// The snapshot is untrusted input: its paths are relative to a declared
    /// home root, bounded, and validated before becoming model entries.
    pub fn from_browser_snapshot_json(input: &str) -> Result<Self, BrowserSnapshotError> {
        let directory = BrowserSnapshot::from_json(input)?.into_directory_model()?;
        Ok(Self::new(directory))
    }

    pub fn new(directory: DirectoryModel) -> Self {
        Self {
            directory,
            launcher: MockLauncher::new(),
        }
    }

    pub fn directory(&self) -> &DirectoryModel {
        &self.directory
    }

    pub fn select_entry(&mut self, index: usize) -> Result<(), DirectoryError> {
        self.directory.select(index)
    }

    pub fn clear_selection(&mut self) {
        self.directory.clear_selection();
    }

    /// Activating a selected .exe creates a preview record only.
    pub fn activate_selected(&mut self) -> Result<LaunchRecord, LaunchError> {
        let selected = self.directory.selected().ok_or(LaunchError::NoSelection)?;
        if !selected.is_windows_executable_candidate() {
            return Err(LaunchError::UnsupportedFileType);
        }
        let request = LaunchRequest::new(selected.path().to_path_buf())?;
        self.launcher.submit(request)
    }

    pub fn cancel_preview(&mut self, id: RequestId) -> Result<LaunchRecord, LaunchError> {
        self.launcher.cancel(id)
    }

    pub fn launch_records(&self) -> impl Iterator<Item = &LaunchRecord> {
        self.launcher.records()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(path: &str, name: &str, kind: EntryKind) -> FileEntry {
        FileEntry::new(PathBuf::from(path), name, kind)
    }

    fn json_temp_root() -> String {
        serde_json::to_string(&std::env::temp_dir()).unwrap()
    }

    #[test]
    fn browser_snapshot_accepts_versioned_relative_entries() {
        let root = json_temp_root();
        let json = format!(r#"{{"protocol_version":1,"root":{root},"current_directory":"Downloads","entries":[{{"name":"setup.exe","relative_path":"setup.exe","kind":"file","size":42}},{{"name":"Docs","relative_path":"Docs","kind":"directory","size":null}}]}}"#);
        let controller = DesktopController::from_browser_snapshot_json(&json).unwrap();
        assert_eq!(controller.directory().current_directory(), &std::env::temp_dir().join("Downloads"));
        assert_eq!(controller.directory().entries()[0].display_name(), "Docs");
        assert_eq!(controller.directory().entries()[1].display_name(), "setup.exe");
    }

    #[test]
    fn browser_snapshot_rejects_parent_traversal_and_nested_paths() {
        let root = json_temp_root();
        let traversal = format!(r#"{{"protocol_version":1,"root":{root},"current_directory":"../outside","entries":[]}}"#);
        assert_eq!(
            BrowserSnapshot::from_json(&traversal).unwrap().into_directory_model(),
            Err(BrowserSnapshotError::CurrentDirectoryContainsTraversal)
        );
        let nested = format!(r#"{{"protocol_version":1,"root":{root},"current_directory":"","entries":[{{"name":"escape","relative_path":"sub/escape","kind":"file","size":null}}]}}"#);
        assert_eq!(
            BrowserSnapshot::from_json(&nested).unwrap().into_directory_model(),
            Err(BrowserSnapshotError::EntryOutsideCurrentDirectory)
        );
    }

    #[test]
    fn browser_snapshot_rejects_unknown_versions_duplicates_and_extra_fields() {
        let root = json_temp_root();
        let version = format!(r#"{{"protocol_version":2,"root":{root},"current_directory":"","entries":[]}}"#);
        assert_eq!(
            BrowserSnapshot::from_json(&version).unwrap().into_directory_model(),
            Err(BrowserSnapshotError::UnsupportedProtocolVersion)
        );
        let duplicate = format!(r#"{{"protocol_version":1,"root":{root},"current_directory":"","entries":[{{"name":"a","relative_path":"a","kind":"file","size":1}},{{"name":"a","relative_path":"a","kind":"file","size":1}}]}}"#);
        assert_eq!(
            BrowserSnapshot::from_json(&duplicate).unwrap().into_directory_model(),
            Err(BrowserSnapshotError::DuplicateEntry)
        );
        let extra = format!(r#"{{"protocol_version":1,"root":{root},"current_directory":"","entries":[],"run_command":"unsafe"}}"#);
        assert_eq!(BrowserSnapshot::from_json(&extra), Err(BrowserSnapshotError::InvalidJson));
    }

    #[test]
    fn controller_connects_selection_to_preview_only_launcher() {
        let view = DirectoryModel::new(
            std::env::temp_dir(),
            vec![
                FileEntry::new(
                    std::env::temp_dir().join("notes.txt"),
                    "notes.txt",
                    EntryKind::File,
                ),
                FileEntry::new(
                    std::env::temp_dir().join("demo.exe"),
                    "demo.exe",
                    EntryKind::File,
                ),
            ],
        );
        let mut controller = DesktopController::new(view);

        assert_eq!(
            controller.activate_selected(),
            Err(LaunchError::NoSelection)
        );
        // DirectoryModel sorts file entries alphabetically, so demo.exe is first.
        controller.select_entry(0).unwrap();
        let preview = controller.activate_selected().unwrap();
        assert_eq!(preview.status, LaunchStatus::PreviewOnly);
        assert!(preview.explanation.contains("no process was created"));
        assert_eq!(controller.launch_records().count(), 1);

        let cancelled = controller.cancel_preview(preview.id).unwrap();
        assert_eq!(cancelled.status, LaunchStatus::Cancelled);
        assert_eq!(controller.launch_records().count(), 1);
    }

    #[test]
    fn controller_rejects_selected_non_executable_files() {
        let view = DirectoryModel::new(
            std::env::temp_dir(),
            vec![FileEntry::new(
                std::env::temp_dir().join("notes.txt"),
                "notes.txt",
                EntryKind::File,
            )],
        );
        let mut controller = DesktopController::new(view);
        controller.select_entry(0).unwrap();
        assert_eq!(
            controller.activate_selected(),
            Err(LaunchError::UnsupportedFileType)
        );
        assert_eq!(controller.launch_records().count(), 0);
    }

    #[test]
    fn directory_view_sorts_folders_first_and_selects_safely() {
        let mut view = DirectoryModel::new(
            PathBuf::from("/home/demo"),
            vec![
                entry("/home/demo/z.exe", "z.exe", EntryKind::File),
                entry("/home/demo/docs", "Docs", EntryKind::Directory),
                entry("/home/demo/a.txt", "a.txt", EntryKind::File),
            ],
        );
        assert_eq!(view.entries()[0].display_name(), "Docs");
        assert_eq!(view.entries()[1].display_name(), "a.txt");
        assert_eq!(view.entries()[2].display_name(), "z.exe");
        assert_eq!(view.select(1), Ok(()));
        assert_eq!(view.selected().unwrap().display_name(), "a.txt");
        assert_eq!(view.select(99), Err(DirectoryError::SelectionOutOfRange));
        view.clear_selection();
        assert!(view.selected().is_none());
    }

    #[test]
    fn display_names_cannot_inject_terminal_control_sequences() {
        let item = entry("/tmp/odd.exe", "report\u{001b}[2J.exe", EntryKind::File);
        assert_eq!(item.display_name(), "report\u{fffd}[2J.exe");
    }

    #[test]
    fn only_absolute_exe_file_paths_are_accepted() {
        assert_eq!(
            LaunchRequest::new(PathBuf::from("relative/app.exe")).unwrap_err(),
            LaunchError::PathMustBeAbsolute
        );
        assert_eq!(
            LaunchRequest::new(std::env::temp_dir().join("app.txt")).unwrap_err(),
            LaunchError::UnsupportedFileType
        );
        assert_eq!(
            LaunchRequest::new(PathBuf::from("")).unwrap_err(),
            LaunchError::EmptyPath
        );
        assert!(LaunchRequest::new(std::env::temp_dir().join("App.EXE")).is_ok());
    }

    #[test]
    fn mock_launcher_never_starts_a_process_and_can_cancel_preview() {
        let mut launcher = MockLauncher::new();
        let record = launcher
            .submit(LaunchRequest::new(std::env::temp_dir().join("game.exe")).unwrap())
            .unwrap();
        assert_eq!(record.status, LaunchStatus::PreviewOnly);
        assert!(record.explanation.contains("no process was created"));
        assert_eq!(launcher.get(record.id), Some(&record));

        let cancelled = launcher.cancel(record.id).unwrap();
        assert_eq!(cancelled.status, LaunchStatus::Cancelled);
        assert!(cancelled.explanation.contains("no process was created"));
        assert_eq!(
            launcher.cancel(record.id),
            Err(LaunchError::RequestAlreadyCancelled)
        );
        assert_eq!(launcher.records().count(), 1);
    }

    #[test]
    fn mock_launcher_rejects_unknown_cancellation_ids() {
        let mut launcher = MockLauncher::new();
        assert_eq!(
            launcher.cancel(RequestId(999)),
            Err(LaunchError::UnknownRequest)
        );
    }

    #[test]
    fn executable_candidate_is_not_a_launch_action() {
        let exe = entry("/home/demo/setup.exe", "setup.exe", EntryKind::File);
        let folder = entry("/home/demo/folder.exe", "folder.exe", EntryKind::Directory);
        assert!(exe.is_windows_executable_candidate());
        assert!(!folder.is_windows_executable_candidate());
    }

    #[test]
    fn default_launcher_starts_request_ids_at_one() {
        let mut launcher = MockLauncher::default();
        let record = launcher
            .submit(LaunchRequest::new(std::env::temp_dir().join("app.exe")).unwrap())
            .unwrap();
        assert_eq!(record.id.get(), 1);
    }

    #[test]
    fn request_id_exhaustion_fails_without_panicking_or_recording() {
        let mut launcher = MockLauncher {
            next_id: u64::MAX,
            records: BTreeMap::new(),
        };
        let request = LaunchRequest::new(std::env::temp_dir().join("app.exe")).unwrap();
        assert_eq!(
            launcher.submit(request),
            Err(LaunchError::RequestIdExhausted)
        );
        assert_eq!(launcher.records().count(), 0);
    }
}
