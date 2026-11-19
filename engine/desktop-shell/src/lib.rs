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
            rank(left.kind)
                .cmp(&rank(right.kind))
                .then_with(|| {
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
        let record = self.records.get_mut(&id).ok_or(LaunchError::UnknownRequest)?;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(path: &str, name: &str, kind: EntryKind) -> FileEntry {
        FileEntry::new(PathBuf::from(path), name, kind)
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
        assert_eq!(
            view.select(99),
            Err(DirectoryError::SelectionOutOfRange)
        );
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
            LaunchRequest::new(PathBuf::from("/tmp/app.txt")).unwrap_err(),
            LaunchError::UnsupportedFileType
        );
        assert_eq!(
            LaunchRequest::new(PathBuf::from("")).unwrap_err(),
            LaunchError::EmptyPath
        );
        assert!(LaunchRequest::new(PathBuf::from("/tmp/App.EXE")).is_ok());
    }

    #[test]
    fn mock_launcher_never_starts_a_process_and_can_cancel_preview() {
        let mut launcher = MockLauncher::new();
        let record = launcher
            .submit(LaunchRequest::new(PathBuf::from("/tmp/game.exe")).unwrap())
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
            .submit(LaunchRequest::new(PathBuf::from("/tmp/app.exe")).unwrap())
            .unwrap();
        assert_eq!(record.id.get(), 1);
    }

    #[test]
    fn request_id_exhaustion_fails_without_panicking_or_recording() {
        let mut launcher = MockLauncher {
            next_id: u64::MAX,
            records: BTreeMap::new(),
        };
        let request = LaunchRequest::new(PathBuf::from("/tmp/app.exe")).unwrap();
        assert_eq!(
            launcher.submit(request),
            Err(LaunchError::RequestIdExhausted)
        );
        assert_eq!(launcher.records().count(), 0);
    }
}
