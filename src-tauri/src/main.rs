mod integrations;
pub mod metrics;
mod runtime;
mod state;

use integrations::config_io::{
    claude_context_failure_reply, claude_settings_snapshot_failure_reply, codex_connection_status,
    codex_status_failure_reply, codex_verify_failure_reply, ApplyConnectionRequest,
    ClaudeContextReply, ClaudeContextSnapshotRequest, ClaudeScopeSelectionRequest,
    ClaudeSettingsSnapshotReply, ClaudeSettingsSnapshotRequest, CodexStatusReply,
    CodexStatusRequest, CodexVerifyManager, CodexVerifyReply, CodexVerifyRequest,
    ConnectionActionReply, OwnershipSnapshotRequest, PlanManager, PlanReply, PreviewRequest,
    UndoConnectionRequest,
};
use integrations::discovery::{
    DiscoveryManager, DiscoveryReply, RememberInstallationChoiceRequest,
    SavedInstallationChoiceReply, SavedInstallationChoiceRequest,
};
use integrations::picker;
use runtime::manager::{IngressReply, RuntimeManager, RuntimeReply};
use state::choices::ChoiceStore;
use state::store::{PreferenceStore, PreferencesReply};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use tauri::Manager;

static EXIT_GATE: ExitGate = ExitGate(AtomicU8::new(0));

struct ExitGate(AtomicU8);

struct ExitAction {
    prevent: bool,
    start_stop: bool,
}

impl ExitGate {
    fn request(&self) -> ExitAction {
        let state = self.0.load(Ordering::SeqCst);
        if state == 2 {
            return ExitAction {
                prevent: false,
                start_stop: false,
            };
        }
        let start_stop = self
            .0
            .compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok();
        ExitAction {
            prevent: true,
            start_stop,
        }
    }

    fn complete(&self) {
        self.0.store(2, Ordering::SeqCst);
    }
}

#[tauri::command]
async fn start_engine(
    state: tauri::State<'_, RuntimeManager>,
    request_id: String,
) -> Result<RuntimeReply, RuntimeReply> {
    let manager = state.inner().clone();
    let failure_id = request_id.clone();
    let result = tauri::async_runtime::spawn_blocking(move || manager.start_engine(request_id))
        .await
        .unwrap_or_else(|_| RuntimeManager::failure_reply(failure_id));
    Ok(result)
}

#[tauri::command]
async fn stop_engine(
    state: tauri::State<'_, RuntimeManager>,
    request_id: String,
    expected_generation: String,
) -> Result<RuntimeReply, RuntimeReply> {
    let manager = state.inner().clone();
    let failure_id = request_id.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        manager.stop_engine(request_id, expected_generation)
    })
    .await
    .unwrap_or_else(|_| RuntimeManager::failure_reply(failure_id));
    Ok(result)
}

#[tauri::command]
fn get_runtime_snapshot(
    state: tauri::State<'_, RuntimeManager>,
    request_id: String,
) -> RuntimeReply {
    state.get_runtime_snapshot(request_id)
}

#[tauri::command]
fn get_ingress_snapshot(
    state: tauri::State<'_, RuntimeManager>,
    request_id: String,
) -> IngressReply {
    state.get_ingress_snapshot(request_id)
}

#[tauri::command]
fn get_preferences(
    state: tauri::State<'_, PreferenceStore>,
    request_id: String,
) -> PreferencesReply {
    state.get_preferences(request_id)
}

#[tauri::command]
fn save_preferences(
    state: tauri::State<'_, PreferenceStore>,
    request_id: String,
    language: String,
    theme: String,
    expected_revision: u64,
    expected_fingerprint: Option<String>,
) -> PreferencesReply {
    state.save_preferences(
        request_id,
        language,
        theme,
        expected_revision,
        expected_fingerprint,
    )
}

#[tauri::command]
fn get_discovery_snapshot(
    state: tauri::State<'_, DiscoveryManager>,
    request_id: String,
) -> DiscoveryReply {
    state.list_snapshot(request_id)
}

#[tauri::command]
async fn scan_discovery(
    state: tauri::State<'_, DiscoveryManager>,
    request_id: String,
) -> Result<DiscoveryReply, DiscoveryReply> {
    let manager = state.inner().clone();
    let failure_id = request_id.clone();
    Ok(
        tauri::async_runtime::spawn_blocking(move || manager.scan(request_id))
            .await
            .unwrap_or_else(|_| DiscoveryManager::failure_reply(failure_id)),
    )
}

#[tauri::command]
async fn get_saved_installation_choice(
    state: tauri::State<'_, DiscoveryManager>,
    request: SavedInstallationChoiceRequest,
) -> Result<SavedInstallationChoiceReply, SavedInstallationChoiceReply> {
    let manager = state.inner().clone();
    let failure_id = request.request_id.clone();
    Ok(
        tauri::async_runtime::spawn_blocking(move || manager.saved_installation_choice(request))
            .await
            .unwrap_or_else(|_| DiscoveryManager::saved_choice_failure_reply(failure_id)),
    )
}

#[tauri::command]
async fn remember_installation_choice(
    state: tauri::State<'_, DiscoveryManager>,
    request: RememberInstallationChoiceRequest,
) -> Result<SavedInstallationChoiceReply, SavedInstallationChoiceReply> {
    let manager = state.inner().clone();
    let failure_id = request.request_id.clone();
    Ok(
        tauri::async_runtime::spawn_blocking(move || manager.remember_installation_choice(request))
            .await
            .unwrap_or_else(|_| DiscoveryManager::saved_choice_failure_reply(failure_id)),
    )
}

#[tauri::command]
async fn preview_connection_plan(
    state: tauri::State<'_, Arc<PlanManager>>,
    request: PreviewRequest,
) -> Result<PlanReply, PlanReply> {
    let manager = Arc::clone(state.inner());
    let failure_id = request.request_id.clone();
    let reply = tauri::async_runtime::spawn_blocking(move || manager.preview(request))
        .await
        .unwrap_or_else(|_| PlanManager::failure_reply(failure_id));
    Ok(reply)
}

#[tauri::command]
async fn apply_connection_plan(
    state: tauri::State<'_, Arc<PlanManager>>,
    request: ApplyConnectionRequest,
) -> Result<ConnectionActionReply, ConnectionActionReply> {
    let manager = Arc::clone(state.inner());
    let failure_id = request.request_id.clone();
    Ok(
        tauri::async_runtime::spawn_blocking(move || manager.apply_connection(request))
            .await
            .unwrap_or_else(|_| PlanManager::action_failure_reply(failure_id)),
    )
}

#[tauri::command]
async fn undo_connection(
    state: tauri::State<'_, Arc<PlanManager>>,
    request: UndoConnectionRequest,
) -> Result<ConnectionActionReply, ConnectionActionReply> {
    let manager = Arc::clone(state.inner());
    let failure_id = request.request_id.clone();
    Ok(
        tauri::async_runtime::spawn_blocking(move || manager.undo_connection(request))
            .await
            .unwrap_or_else(|_| PlanManager::action_failure_reply(failure_id)),
    )
}

#[tauri::command]
async fn get_connection_ownership(
    state: tauri::State<'_, Arc<PlanManager>>,
    request: OwnershipSnapshotRequest,
) -> Result<ConnectionActionReply, ConnectionActionReply> {
    let manager = Arc::clone(state.inner());
    let failure_id = request.request_id.clone();
    Ok(
        tauri::async_runtime::spawn_blocking(move || manager.ownership_snapshot(request))
            .await
            .unwrap_or_else(|_| PlanManager::action_failure_reply(failure_id)),
    )
}

#[tauri::command]
async fn get_codex_connection_status(
    plans: tauri::State<'_, Arc<PlanManager>>,
    runtime: tauri::State<'_, RuntimeManager>,
    request: CodexStatusRequest,
) -> Result<CodexStatusReply, CodexStatusReply> {
    let manager = Arc::clone(plans.inner());
    let runtime = runtime.inner().clone();
    let failure_id = request.request_id.clone();
    Ok(tauri::async_runtime::spawn_blocking(move || {
        codex_connection_status(&manager, &runtime, request)
    })
    .await
    .unwrap_or_else(|_| codex_status_failure_reply(failure_id)))
}

#[tauri::command]
async fn verify_codex_connection(
    plans: tauri::State<'_, Arc<PlanManager>>,
    runtime: tauri::State<'_, RuntimeManager>,
    verifier: tauri::State<'_, CodexVerifyManager>,
    request: CodexVerifyRequest,
) -> Result<CodexVerifyReply, CodexVerifyReply> {
    let plans = Arc::clone(plans.inner());
    let runtime = runtime.inner().clone();
    let verifier = verifier.inner().clone();
    let failure_id = request.request_id.clone();
    Ok(
        tauri::async_runtime::spawn_blocking(move || verifier.verify(&plans, &runtime, request))
            .await
            .unwrap_or_else(|_| codex_verify_failure_reply(failure_id)),
    )
}

#[tauri::command]
async fn get_claude_context_snapshot(
    state: tauri::State<'_, Arc<PlanManager>>,
    request: ClaudeContextSnapshotRequest,
) -> Result<ClaudeContextReply, ClaudeContextReply> {
    let manager = Arc::clone(state.inner());
    let failure_id = request.request_id.clone();
    Ok(
        tauri::async_runtime::spawn_blocking(move || manager.claude_context_snapshot(request))
            .await
            .unwrap_or_else(|_| claude_context_failure_reply(failure_id)),
    )
}

#[tauri::command]
async fn select_claude_scope(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, Arc<PlanManager>>,
    request: ClaudeScopeSelectionRequest,
) -> Result<ClaudeContextReply, ClaudeContextReply> {
    if window.label() != "main" {
        return Ok(claude_context_failure_reply(request.request_id));
    }
    let manager = Arc::clone(state.inner());
    let failure_id = request.request_id.clone();
    Ok(
        tauri::async_runtime::spawn_blocking(move || manager.select_claude_scope(request))
            .await
            .unwrap_or_else(|_| claude_context_failure_reply(failure_id)),
    )
}

#[tauri::command]
async fn get_claude_settings_snapshot(
    state: tauri::State<'_, Arc<PlanManager>>,
    request: ClaudeSettingsSnapshotRequest,
) -> Result<ClaudeSettingsSnapshotReply, ClaudeSettingsSnapshotReply> {
    let manager = Arc::clone(state.inner());
    let failure_id = request.request_id.clone();
    Ok(
        tauri::async_runtime::spawn_blocking(move || manager.claude_settings_snapshot(request))
            .await
            .unwrap_or_else(|_| claude_settings_snapshot_failure_reply(failure_id)),
    )
}

#[tauri::command]
async fn select_client_file(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, DiscoveryManager>,
    request_id: String,
    client_kind: String,
) -> Result<DiscoveryReply, DiscoveryReply> {
    let manager = state.inner().clone();
    let failure_id = request_id.clone();
    let owner_hwnd = (window.label() == "main")
        .then(|| window.hwnd().ok())
        .flatten()
        .map(|hwnd| hwnd.0 as usize);
    let reply = tauri::async_runtime::spawn_blocking(move || {
        let selector_thread = std::thread::Builder::new()
            .name("headroom-file-picker-sta".to_owned())
            .spawn(move || {
                manager.select_client_file_with(request_id, client_kind, |kind| {
                    owner_hwnd
                        .ok_or(())
                        .and_then(|owner_hwnd| picker::select_file(owner_hwnd, kind))
                })
            });
        selector_thread.map_err(|_| ())?.join().map_err(|_| ())
    })
    .await
    .ok()
    .and_then(Result::ok)
    .unwrap_or_else(|| DiscoveryManager::failure_reply(failure_id));
    Ok(reply)
}

fn main() {
    let app = tauri::Builder::default()
        .setup(|app| {
            let resource_dir = app.path().resource_dir().ok();
            let app_data_dir = app.path().app_data_dir().ok();
            let app_local_data_dir = app.path().app_local_data_dir().ok();
            let runtime = RuntimeManager::new(resource_dir, app_data_dir);
            let choice_store = ChoiceStore::for_app_local_data(app_local_data_dir.clone()).ok();
            let discovery = DiscoveryManager::with_choice_store(choice_store.clone());
            let claude_picker_owner = app
                .get_webview_window("main")
                .and_then(|window| window.hwnd().ok())
                .map(|hwnd| hwnd.0 as usize);
            let ownership_store = integrations::config_io::ownership_store_for_app_local_data(
                app_local_data_dir.clone(),
            )
            .ok();
            let plans = Arc::new(
                PlanManager::new(discovery.clone(), runtime.clone())
                    .with_optional_ownership_store(ownership_store)
                    .with_claude_choice_store(choice_store)
                    .with_claude_project_picker_hint(move |initial_directory| {
                        let owner_hwnd = claude_picker_owner.ok_or(())?;
                        std::thread::Builder::new()
                            .name("headroom-claude-folder-picker-sta".to_owned())
                            .spawn(move || {
                                picker::select_project_directory(owner_hwnd, initial_directory)
                            })
                            .map_err(|_| ())?
                            .join()
                            .map_err(|_| ())?
                    }),
            );
            let codex_verifier = CodexVerifyManager::new(app_local_data_dir);
            app.manage(runtime);
            app.manage(PreferenceStore::new(app.path().app_data_dir().ok()));
            app.manage(discovery);
            app.manage(plans);
            app.manage(codex_verifier);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            start_engine,
            stop_engine,
            get_runtime_snapshot,
            get_ingress_snapshot,
            get_preferences,
            save_preferences,
            get_discovery_snapshot,
            scan_discovery,
            get_saved_installation_choice,
            remember_installation_choice,
            select_client_file,
            preview_connection_plan,
            apply_connection_plan,
            undo_connection,
            get_connection_ownership,
            get_claude_context_snapshot,
            select_claude_scope,
            get_claude_settings_snapshot,
            get_codex_connection_status,
            verify_codex_connection
        ])
        .build(tauri::generate_context!())
        .expect("failed to build Headroom desktop");

    app.run(|app_handle, event| {
        #[cfg(debug_assertions)]
        if matches!(
            &event,
            tauri::RunEvent::WindowEvent {
                event: tauri::WindowEvent::CloseRequested { .. },
                ..
            }
        ) {
            eprintln!("headroom_exit event=window_close");
        }
        if let tauri::RunEvent::ExitRequested { code, api, .. } = event {
            let action = EXIT_GATE.request();
            #[cfg(debug_assertions)]
            eprintln!(
                "headroom_exit event=exit_requested prevent={} start={}",
                action.prevent as u8, action.start_stop as u8
            );
            if action.prevent {
                api.prevent_exit();
            }
            if action.start_stop {
                let app_handle = app_handle.clone();
                std::thread::spawn(move || {
                    #[cfg(debug_assertions)]
                    let exit_start = std::time::Instant::now();
                    #[cfg(debug_assertions)]
                    eprintln!("headroom_exit event=verifier_begin");
                    let verifier_clean =
                        app_handle.state::<CodexVerifyManager>().inner().shutdown();
                    #[cfg(debug_assertions)]
                    eprintln!(
                        "headroom_exit event=verifier_done clean={} elapsed_ms={}",
                        verifier_clean as u8,
                        exit_start.elapsed().as_millis()
                    );
                    let runtime_clean = app_handle.state::<RuntimeManager>().shutdown();
                    #[cfg(debug_assertions)]
                    eprintln!(
                        "headroom_exit event=runtime_done clean={} elapsed_ms={}",
                        runtime_clean as u8,
                        exit_start.elapsed().as_millis()
                    );
                    let clean = verifier_clean && runtime_clean;
                    let exit_code = if clean { code.unwrap_or(0) } else { 1 };
                    EXIT_GATE.complete();
                    #[cfg(debug_assertions)]
                    eprintln!(
                        "headroom_exit event=dispatch code={} elapsed_ms={}",
                        exit_code,
                        exit_start.elapsed().as_millis()
                    );
                    app_handle.exit(exit_code);
                });
            }
        }
    });
}

#[cfg(test)]
mod exit_tests {
    use super::ExitGate;
    use std::sync::atomic::AtomicU8;

    #[test]
    fn repeated_exit_requests_stay_blocked_until_shutdown_completes() {
        let gate = ExitGate(AtomicU8::new(0));
        let first = gate.request();
        assert!(first.prevent);
        assert!(first.start_stop);

        let during_stop = gate.request();
        assert!(during_stop.prevent);
        assert!(!during_stop.start_stop);

        gate.complete();
        let after_stop = gate.request();
        assert!(!after_stop.prevent);
        assert!(!after_stop.start_stop);
    }
}
