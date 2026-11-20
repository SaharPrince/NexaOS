//! One-shot JSON adapter for the preview-only desktop controller.
//!
//! This process never opens or executes the selected file. It only validates
//! a bounded browser snapshot and returns controller state as JSON.
use std::io::{self, Read};

use nexa_desktop_shell::{BrowserSnapshot, DesktopController};
use serde::Deserialize;
use serde_json::{json, Value};

const MAX_REQUEST_BYTES: usize = 1024 * 1024;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ControllerCommand {
    protocol_version: u32,
    action: ControllerAction,
    snapshot: BrowserSnapshot,
    selected_relative_path: Option<std::path::PathBuf>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ControllerAction {
    InspectSnapshot,
    PreviewSelected,
}

fn handle(input: &[u8]) -> Result<Value, String> {
    let command: ControllerCommand =
        serde_json::from_slice(input).map_err(|_| "invalid_request_json".to_owned())?;
    if command.protocol_version != 1 {
        return Err("unsupported_protocol_version".to_owned());
    }

    let directory = command
        .snapshot
        .into_directory_model()
        .map_err(|error| format!("invalid_snapshot:{error:?}"))?;
    let mut controller = DesktopController::new(directory);

    match command.action {
        ControllerAction::InspectSnapshot => Ok(json!({
            "protocol_version": 1,
            "status": "snapshot_loaded",
            "entry_count": controller.directory().entries().len(),
            "current_directory": controller.directory().current_directory(),
        })),
        ControllerAction::PreviewSelected => {
            let relative_path = command
                .selected_relative_path
                .as_deref()
                .ok_or_else(|| "selected_path_required".to_owned())?;
            controller
                .select_relative_path(relative_path)
                .map_err(|error| format!("selection_rejected:{error:?}"))?;
            let record = controller
                .activate_selected()
                .map_err(|error| format!("preview_rejected:{error:?}"))?;
            Ok(json!({
                "protocol_version": 1,
                "status": "preview_only",
                "request_id": record.id.get(),
                "explanation": record.explanation,
            }))
        }
    }
}

fn main() {
    let mut input = Vec::new();
    let read_result = io::stdin()
        .take((MAX_REQUEST_BYTES + 1) as u64)
        .read_to_end(&mut input);
    let result = match read_result {
        Err(_) => Err("request_read_failed".to_owned()),
        Ok(_) if input.len() > MAX_REQUEST_BYTES => Err("request_too_large".to_owned()),
        Ok(_) => handle(&input),
    };

    match result {
        Ok(response) => println!("{response}"),
        Err(code) => {
            println!("{}", json!({"protocol_version": 1, "status": "error", "error": code}));
            std::process::exit(2);
        }
    }
}
