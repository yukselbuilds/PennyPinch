#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod compatibility;
mod desktop;
mod export;
mod insights;
mod integrations;
pub mod metrics;
mod runtime;
mod state;

use integrations::accounts::{AccountManager, Reply as AccountReply, Request as AccountRequest};
use integrations::config_io::{
    claude_context_failure_reply, claude_settings_snapshot_failure_reply, codex_connection_status,
    codex_status_failure_reply, codex_verify_failure_reply, ApplyConnectionRequest,
    ArchiveRetainedRequest, ClaudeContextReply, ClaudeContextSnapshotRequest,
    ClaudeScopeSelectionRequest, ClaudeSettingsSnapshotReply, ClaudeSettingsSnapshotRequest,
    CodexStatusReply, CodexStatusRequest, CodexVerifyManager, CodexVerifyReply, CodexVerifyRequest,
    ConnectionActionReply, OwnershipSnapshotRequest, PlanManager, PlanReply, PreviewRequest,
    RetainedReply, RetainedRequest, UndoConnectionRequest,
};
use integrations::discovery::{
    DiscoveryManager, DiscoveryReply, RememberInstallationChoiceRequest,
    SavedInstallationChoiceReply, SavedInstallationChoiceRequest,
};
use integrations::picker;
use metrics::activity::{
    ActivityDetailReply, ActivityDetailRequest, ActivityListReply, ActivityQueryRequest,
    ActivityReply,
};
use metrics::collector::{OverviewManager, OverviewReply};
use metrics::store::{MetricsError, MetricsQueryRequest, MetricsReply, MetricsStore};
use runtime::manager::{IngressReply, RuntimeManager, RuntimeReply};
use state::choices::ChoiceStore;
use state::store::{OperatingPreferences, PreferenceStore, PreferencesReply};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use tauri::Manager;

fn picker_owner_handle(window: &tauri::WebviewWindow) -> Option<usize> {
    #[cfg(windows)]
    {
        window.hwnd().ok().map(|handle| handle.0 as usize)
    }
    #[cfg(target_os = "macos")]
    {
        window.ns_window().ok().map(|handle| handle as usize)
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = window;
        None
    }
}

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
async fn set_optimization(
    state: tauri::State<'_, RuntimeManager>,
    request_id: String,
    expected_generation: String,
    expected_revision: u64,
    enabled: bool,
) -> Result<RuntimeReply, RuntimeReply> {
    let manager = state.inner().clone();
    let failure_id = request_id.clone();
    Ok(tauri::async_runtime::spawn_blocking(move || {
        manager.set_optimization(request_id, expected_generation, expected_revision, enabled)
    })
    .await
    .unwrap_or_else(|_| RuntimeManager::failure_reply(failure_id)))
}

#[tauri::command]
fn get_ingress_snapshot(
    state: tauri::State<'_, RuntimeManager>,
    request_id: String,
) -> IngressReply {
    state.get_ingress_snapshot(request_id)
}

#[tauri::command]
async fn get_recovery_snapshot(
    recovery: tauri::State<'_, runtime::recovery::RecoveryManager>,
    runtime: tauri::State<'_, RuntimeManager>,
    discovery: tauri::State<'_, DiscoveryManager>,
    plans: tauri::State<'_, Arc<PlanManager>>,
    overview: tauri::State<'_, OverviewManager>,
    request: runtime::recovery::Request,
) -> Result<runtime::recovery::Reply, runtime::recovery::Reply> {
    let (recovery, runtime, discovery, plans, overview) = (
        recovery.inner().clone(),
        runtime.inner().clone(),
        discovery.inner().clone(),
        plans.inner().clone(),
        overview.inner().clone(),
    );
    let id = request.request_id.clone();
    Ok(tauri::async_runtime::spawn_blocking(move || {
        recovery.read(request, &runtime, &discovery, &plans, &overview)
    })
    .await
    .unwrap_or_else(|_| runtime::recovery::failure(id, "unavailable")))
}

#[tauri::command]
async fn get_compatibility_snapshot(
    catalog: tauri::State<'_, compatibility::catalog::CatalogManager>,
    runtime: tauri::State<'_, RuntimeManager>,
    discovery: tauri::State<'_, DiscoveryManager>,
    plans: tauri::State<'_, Arc<PlanManager>>,
    request: compatibility::catalog::Request,
) -> Result<compatibility::catalog::Reply, compatibility::catalog::Reply> {
    let (catalog, runtime, discovery, plans) = (
        catalog.inner().clone(),
        runtime.inner().clone(),
        discovery.inner().clone(),
        plans.inner().clone(),
    );
    let id = request.request_id.clone();
    Ok(tauri::async_runtime::spawn_blocking(move || {
        catalog.read(request, &runtime, &discovery, &plans)
    })
    .await
    .unwrap_or_else(|_| compatibility::catalog::failure(id, "unavailable")))
}

#[tauri::command]
async fn get_metrics_summary(
    state: tauri::State<'_, MetricsStore>,
    request: MetricsQueryRequest,
) -> Result<MetricsReply, MetricsReply> {
    let store = state.inner().clone();
    let request_id = request.request_id.clone();
    Ok(
        tauri::async_runtime::spawn_blocking(move || store.query(request))
            .await
            .unwrap_or_else(|_| MetricsStore::failure(request_id, MetricsError::Unavailable)),
    )
}

#[tauri::command]
async fn get_overview_snapshot(
    state: tauri::State<'_, OverviewManager>,
    reports: tauri::State<'_, export::report::ReportManager>,
    discovery: tauri::State<'_, DiscoveryManager>,
    request: MetricsQueryRequest,
    include_recommendations: Option<bool>,
) -> Result<OverviewReply, OverviewReply> {
    let overview = state.inner().clone();
    let discovery = discovery.inner().clone();
    let include_recommendations = include_recommendations.unwrap_or(false);
    let request_id = request.request_id.clone();
    let reply = tauri::async_runtime::spawn_blocking(move || {
        let mut reply = overview.read(request, include_recommendations);
        if include_recommendations {
            if let OverviewReply::Snapshot(snapshot) = &mut reply {
                if let Some(scan) = discovery.recovery_scan() {
                    snapshot
                        .recommendations
                        .extend(insights::rules::installation_checks(
                            scan.installations.iter().map(|(installation, freshness)| {
                                (installation.client_kind, *freshness)
                            }),
                        ));
                }
            }
        }
        reply
    })
    .await
    .unwrap_or_else(|_| {
        OverviewReply::Error(MetricsStore::error_detail(
            request_id,
            MetricsError::Unavailable,
        ))
    });
    if let OverviewReply::Snapshot(snapshot) = &reply {
        reports.remember(snapshot);
    }
    Ok(reply)
}

#[tauri::command]
async fn export_report(
    app: tauri::AppHandle,
    reports: tauri::State<'_, export::report::ReportManager>,
    runtime: tauri::State<'_, RuntimeManager>,
    request: export::report::Request,
) -> Result<export::report::Reply, export::report::Reply> {
    use export::report::{failure, Error};
    let id = request.request_id.clone();
    if app.state::<desktop::lifecycle::Lifecycle>().is_exiting() {
        return Ok(failure(&id, Error::Busy));
    }
    let job = match reports.begin(request) {
        Ok(job) => job,
        Err(error) => return Ok(failure(&id, error)),
    };
    let snapshot = runtime.get_runtime_snapshot(uuid::Uuid::new_v4().to_string());
    let owner = app
        .get_webview_window("main")
        .as_ref()
        .and_then(picker_owner_handle);
    Ok(tauri::async_runtime::spawn_blocking(move || {
        job.save(&snapshot, |artifact| {
            owner
                .ok_or(())
                .and_then(|owner| picker::save_report(owner, artifact))
        })
    })
    .await
    .unwrap_or_else(|_| failure(&id, Error::Unavailable)))
}

#[tauri::command]
async fn get_activity_snapshot(
    state: tauri::State<'_, OverviewManager>,
    request: ActivityQueryRequest,
) -> Result<ActivityListReply, ActivityListReply> {
    let manager = state.inner().clone();
    let id = request.request_id.clone();
    Ok(
        tauri::async_runtime::spawn_blocking(move || manager.read_activity(request))
            .await
            .unwrap_or_else(|_| ActivityReply::error(id, MetricsError::Unavailable)),
    )
}
#[tauri::command]
async fn get_activity_detail(
    state: tauri::State<'_, OverviewManager>,
    request: ActivityDetailRequest,
) -> Result<ActivityDetailReply, ActivityDetailReply> {
    let manager = state.inner().clone();
    let id = request.request_id.clone();
    Ok(
        tauri::async_runtime::spawn_blocking(move || manager.read_activity_detail(request))
            .await
            .unwrap_or_else(|_| ActivityReply::error(id, MetricsError::Unavailable)),
    )
}
#[tauri::command]
async fn manage_updates(
    app: tauri::AppHandle,
    state: tauri::State<'_, desktop::update_flow::UpdateFlow>,
    runtime: tauri::State<'_, RuntimeManager>,
    lifecycle: tauri::State<'_, desktop::lifecycle::Lifecycle>,
    request: desktop::update_flow::Request,
) -> Result<desktop::update_flow::Reply, desktop::update_flow::Reply> {
    use desktop::update_flow::Failure;
    if lifecycle.is_exiting() {
        return Ok(state.error(&request.request_id, Failure::Busy));
    }
    let id = request.request_id.clone();
    let flow = state.inner().clone();
    let runtime = runtime.inner().clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        #[cfg(all(windows, not(debug_assertions)))]
        {
            use desktop::update_install::Installation;
            let installed = Installation::new(app.config(), app.package_info().version.clone());
            if request.changes_installation() {
                let Some(installed) = installed else {
                    return flow.error(&request.request_id, Failure::Unavailable);
                };
                if !runtime.reserve_for_update() {
                    return flow.error(&request.request_id, Failure::EngineRunning);
                }
                let launch = flow
                    .prepare_install(&request, &installed)
                    .and_then(|launch| flow.queue_install(launch));
                if let Err(error) = launch {
                    runtime.release_update_reservation();
                    return flow.error(&request.request_id, error);
                }
                let began = app
                    .state::<desktop::lifecycle::Lifecycle>()
                    .begin_update(|| {
                        app.state::<export::report::ReportManager>()
                            .hold_for_exit(|| app.state::<Arc<PlanManager>>().hold_for_exit())
                            && flow.arm_install()
                    });
                if !began {
                    flow.cancel_install();
                    runtime.release_update_reservation();
                    return flow.error(&request.request_id, Failure::Busy);
                }
                let reply = flow.exiting(request.request_id);
                app.exit(0);
                return reply;
            }
            let mut reply = flow.handle(request, || runtime.update_engine_stopped());
            if let Some(installed) = installed {
                reply.installed_actions(&installed, runtime.update_engine_stopped());
            }
            reply
        }
        #[cfg(not(all(windows, not(debug_assertions))))]
        {
            let _ = app;
            flow.handle(request, || runtime.update_engine_stopped())
        }
    })
    .await
    .unwrap_or_else(|_| state.error(&id, Failure::Unavailable));
    Ok(result)
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
    app: tauri::AppHandle,
    state: tauri::State<'_, PreferenceStore>,
    request_id: String,
    language: String,
    theme: String,
    operating: Option<OperatingPreferences>,
    show_recommendations: Option<bool>,
    expected_revision: u64,
    expected_fingerprint: Option<String>,
) -> PreferencesReply {
    let reply = state.save_with_operating(
        request_id,
        language,
        theme,
        operating,
        show_recommendations,
        expected_revision,
        expected_fingerprint,
    );
    if let PreferencesReply::Snapshot(ref snapshot) = reply {
        app.state::<desktop::lifecycle::Lifecycle>()
            .set_close_to_tray(snapshot.preferences.operating.close_to_tray);
        desktop::lifecycle::notify(&app);
        if let Some(tray) = app.try_state::<desktop::tray::TrayController>() {
            tray.set_language(&snapshot.preferences.language);
        }
    }
    reply
}

#[tauri::command]
fn get_operating_snapshot(
    state: tauri::State<'_, RuntimeManager>,
    startup: tauri::State<'_, desktop::autostart::Autostart>,
    request_id: String,
) -> serde_json::Value {
    desktop::settings::read(&request_id, &state, &startup)
}
#[tauri::command]
fn set_autostart(
    app: tauri::AppHandle,
    state: tauri::State<'_, RuntimeManager>,
    startup: tauri::State<'_, desktop::autostart::Autostart>,
    request_id: String,
    enabled: bool,
    expected_fingerprint: Option<String>,
) -> serde_json::Value {
    if app.state::<desktop::lifecycle::Lifecycle>().is_exiting() {
        return desktop::settings::error(&request_id, "busy");
    }
    desktop::settings::set_startup(
        &request_id,
        enabled,
        expected_fingerprint.as_deref(),
        &state,
        &startup,
    )
}

#[tauri::command]
fn get_desktop_state(
    state: tauri::State<'_, desktop::lifecycle::Lifecycle>,
    request_id: String,
) -> desktop::lifecycle::Reply {
    state.snapshot(&request_id)
}
#[tauri::command]
fn request_desktop_exit(
    app: tauri::AppHandle,
    state: tauri::State<'_, desktop::lifecycle::Lifecycle>,
    request_id: String,
) -> desktop::lifecycle::Reply {
    let reply = state.request(&request_id);
    desktop::lifecycle::notify(&app);
    reply
}
#[tauri::command]
fn cancel_desktop_exit(
    app: tauri::AppHandle,
    state: tauri::State<'_, desktop::lifecycle::Lifecycle>,
    request_id: String,
    exit_token: String,
) -> desktop::lifecycle::Reply {
    let reply = state.cancel(&request_id, &exit_token);
    desktop::lifecycle::notify(&app);
    reply
}
#[tauri::command]
fn confirm_desktop_exit(
    app: tauri::AppHandle,
    state: tauri::State<'_, desktop::lifecycle::Lifecycle>,
    plans: tauri::State<'_, Arc<PlanManager>>,
    reports: tauri::State<'_, export::report::ReportManager>,
    request_id: String,
    exit_token: String,
) -> desktop::lifecycle::Reply {
    let (reply, started) = state.confirm(&request_id, &exit_token, || {
        reports.hold_for_exit(|| plans.hold_for_exit())
    });
    desktop::lifecycle::notify(&app);
    if started {
        app.exit(0);
    }
    reply
}

#[tauri::command]
async fn manage_account(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, AccountManager>,
    discovery: tauri::State<'_, DiscoveryManager>,
    request: AccountRequest,
) -> Result<AccountReply, AccountReply> {
    if window.label() != "main" {
        return Ok(integrations::accounts::failure(
            &request.request_id,
            "unavailable",
        ));
    }
    let manager = state.inner().clone();
    let discovery = discovery.inner().clone();
    let failure_id = request.request_id.clone();
    Ok(
        tauri::async_runtime::spawn_blocking(move || manager.execute(request, &discovery))
            .await
            .unwrap_or_else(|_| integrations::accounts::failure(&failure_id, "unavailable")),
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
async fn get_retained_config_versions(
    state: tauri::State<'_, Arc<PlanManager>>,
    request: RetainedRequest,
) -> Result<RetainedReply, RetainedReply> {
    let manager = Arc::clone(state.inner());
    let id = request.request_id.clone();
    Ok(
        tauri::async_runtime::spawn_blocking(move || manager.retained_versions(request))
            .await
            .unwrap_or_else(|_| PlanManager::retained_failure_reply(id)),
    )
}

#[tauri::command]
async fn archive_retained_config_versions(
    state: tauri::State<'_, Arc<PlanManager>>,
    request: ArchiveRetainedRequest,
) -> Result<RetainedReply, RetainedReply> {
    let manager = Arc::clone(state.inner());
    let id = request.request_id.clone();
    Ok(
        tauri::async_runtime::spawn_blocking(move || manager.archive_retained_versions(request))
            .await
            .unwrap_or_else(|_| PlanManager::retained_failure_reply(id)),
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
        .then(|| picker_owner_handle(&window))
        .flatten();
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
    let context = tauri::generate_context!();
    #[cfg(all(windows, not(debug_assertions)))]
    desktop::update_install::enter(context.config(), &context.package_info().version);
    #[cfg(all(windows, not(debug_assertions)))]
    let _desktop_use = desktop::uninstall::enter(context.config());
    let app = tauri::Builder::default()
        .setup(|app| {
            let resource_dir = app.path().resource_dir().ok();
            let app_local_data_dir = app.path().app_local_data_dir().ok();
            let preferences = PreferenceStore::new(app_local_data_dir.clone());
            let runtime = RuntimeManager::with_preferences(
                resource_dir,
                app_local_data_dir.clone(),
                preferences.clone(),
            );
            let choice_store = ChoiceStore::for_app_local_data(app_local_data_dir.clone()).ok();
            let discovery = DiscoveryManager::with_choice_store(choice_store.clone());
            let claude_picker_owner = app
                .get_webview_window("main")
                .as_ref()
                .and_then(picker_owner_handle);
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
            let codex_verifier = CodexVerifyManager::new(app_local_data_dir.clone());
            app.manage(AccountManager::new(app_local_data_dir.clone()));
            app.manage(desktop::update_flow::UpdateFlow::new(
                app.config().identifier.clone(),
                app.package_info().version.clone(),
                app_local_data_dir.clone(),
            ));
            let metrics =
                MetricsStore::new(app_local_data_dir).with_preferences(preferences.clone());
            let overview = OverviewManager::new(runtime.clone(), metrics.clone());
            app.manage(runtime);
            app.manage(preferences);
            app.manage(discovery);
            app.manage(plans);
            app.manage(codex_verifier);
            app.manage(metrics);
            app.manage(overview);
            app.manage(export::report::ReportManager::default());
            app.manage(runtime::recovery::RecoveryManager::default());
            app.manage(compatibility::catalog::CatalogManager::default());
            app.manage(desktop::lifecycle::Lifecycle::default());
            app.manage(desktop::autostart::Autostart::new());
            let close_to_tray = app
                .state::<PreferenceStore>()
                .operating()
                .is_ok_and(|p| p.close_to_tray);
            app.state::<desktop::lifecycle::Lifecycle>()
                .set_close_to_tray(close_to_tray);
            let language = match app
                .state::<PreferenceStore>()
                .get_preferences(uuid::Uuid::new_v4().to_string())
            {
                PreferencesReply::Snapshot(snapshot) => snapshot.preferences.language,
                _ => "tr".to_owned(),
            };
            app.manage(desktop::tray::TrayController::install(
                app.handle(),
                &language,
            ));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            start_engine,
            stop_engine,
            set_optimization,
            get_runtime_snapshot,
            get_desktop_state,
            request_desktop_exit,
            cancel_desktop_exit,
            confirm_desktop_exit,
            get_ingress_snapshot,
            get_recovery_snapshot,
            get_compatibility_snapshot,
            get_metrics_summary,
            get_overview_snapshot,
            export_report,
            get_activity_snapshot,
            get_activity_detail,
            manage_updates,
            get_preferences,
            save_preferences,
            get_operating_snapshot,
            set_autostart,
            get_discovery_snapshot,
            manage_account,
            scan_discovery,
            get_saved_installation_choice,
            remember_installation_choice,
            select_client_file,
            preview_connection_plan,
            apply_connection_plan,
            undo_connection,
            get_connection_ownership,
            get_retained_config_versions,
            archive_retained_config_versions,
            get_claude_context_snapshot,
            select_claude_scope,
            get_claude_settings_snapshot,
            get_codex_connection_status,
            verify_codex_connection
        ])
        .build(context)
        .expect("failed to build Headroom desktop");

    app.run(|app_handle, event| {
        if let tauri::RunEvent::WindowEvent {
            label,
            event: tauri::WindowEvent::CloseRequested { api, .. },
            ..
        } = &event
        {
            if label == "main" && EXIT_GATE.0.load(Ordering::SeqCst) != 2 {
                api.prevent_close();
                let state = app_handle.state::<desktop::lifecycle::Lifecycle>();
                if state.hide_on_close() {
                    if let Some(window) = app_handle.get_webview_window("main") {
                        let _ = window.hide();
                    }
                    desktop::lifecycle::notify(app_handle);
                } else if !state.is_exiting() {
                    desktop::lifecycle::request_exit(app_handle);
                }
            }
        }
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
            if !app_handle
                .state::<desktop::lifecycle::Lifecycle>()
                .is_exiting()
            {
                api.prevent_exit();
                desktop::lifecycle::request_exit(app_handle);
                return;
            }
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
                    let accounts_clean = app_handle.state::<AccountManager>().shutdown();
                    #[cfg(debug_assertions)]
                    eprintln!(
                        "headroom_exit event=verifier_done clean={} elapsed_ms={}",
                        verifier_clean as u8,
                        exit_start.elapsed().as_millis()
                    );
                    let collector_clean = app_handle.state::<OverviewManager>().shutdown();
                    let runtime_clean = app_handle.state::<RuntimeManager>().shutdown();
                    let tray_clean = app_handle
                        .state::<desktop::tray::TrayController>()
                        .shutdown(&app_handle);
                    #[cfg(debug_assertions)]
                    eprintln!(
                        "headroom_exit event=runtime_done clean={} elapsed_ms={}",
                        runtime_clean as u8,
                        exit_start.elapsed().as_millis()
                    );
                    let clean = verifier_clean
                        && accounts_clean
                        && collector_clean
                        && runtime_clean
                        && tray_clean;
                    #[cfg(all(windows, not(debug_assertions)))]
                    let clean = app_handle
                        .state::<desktop::update_flow::UpdateFlow>()
                        .finish_install(clean)
                        && clean;
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
