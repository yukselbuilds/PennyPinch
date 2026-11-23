use super::*;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RetainedRequest {
    pub schema_version: u8,
    pub request_id: String,
    pub client_kind: String,
    pub scope: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ArchiveRetainedRequest {
    pub schema_version: u8,
    pub request_id: String,
    pub client_kind: String,
    pub scope: String,
    pub expected_review_fingerprint: String,
}

#[derive(Serialize)]
#[serde(untagged)]
pub(crate) enum RetainedReply {
    Snapshot(RetainedSnapshot),
    Error(RetainedFailure),
}

#[derive(Serialize)]
pub(crate) struct RetainedSnapshot {
    schema_version: u8,
    request_id: String,
    status: &'static str,
    client_kind: String,
    scope: String,
    supported: bool,
    review_fingerprint: Option<String>,
    active_records: usize,
    selected_records: usize,
    unresolved_records: usize,
    changed_versions: usize,
    completed_archives: usize,
    record_limit: usize,
    archive_limit: usize,
    can_archive: bool,
}

#[derive(Serialize)]
pub(crate) struct RetainedFailure {
    schema_version: u8,
    request_id: String,
    status: &'static str,
    error: RetainedError,
}

#[derive(Serialize)]
struct RetainedError {
    code: &'static str,
    retryable: bool,
    message_key: String,
    diagnostic_id: Option<()>,
}

fn failure(id: &str, code: &'static str) -> RetainedReply {
    RetainedReply::Error(RetainedFailure {
        schema_version: 1,
        request_id: if valid_uuid(id) {
            id.to_owned()
        } else {
            ZERO_UUID.to_owned()
        },
        status: "error",
        error: RetainedError {
            code,
            retryable: !matches!(code, "invalid_input" | "archive_limit"),
            message_key: format!("retained.{code}"),
            diagnostic_id: None,
        },
    })
}

impl PlanManager {
    pub(crate) fn retained_failure_reply(id: String) -> RetainedReply {
        failure(&id, "unavailable")
    }
    pub(crate) fn retained_versions(&self, request: RetainedRequest) -> RetainedReply {
        self.retained_action(request, None)
    }

    pub(crate) fn archive_retained_versions(
        &self,
        request: ArchiveRetainedRequest,
    ) -> RetainedReply {
        let ArchiveRetainedRequest {
            schema_version,
            request_id,
            client_kind,
            scope,
            expected_review_fingerprint,
        } = request;
        if expected_review_fingerprint.len() != 64
            || !expected_review_fingerprint
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return failure(&request_id, "invalid_input");
        }
        self.retained_action(
            RetainedRequest {
                schema_version,
                request_id,
                client_kind,
                scope,
            },
            Some(expected_review_fingerprint),
        )
    }

    fn retained_action(&self, request: RetainedRequest, expected: Option<String>) -> RetainedReply {
        let kind = match request.client_kind.as_str() {
            "codex" => ClientKind::Codex,
            "claude_code" => ClientKind::ClaudeCode,
            "opencode_v1" => ClientKind::OpenCodeV1,
            _ => return failure(&request.request_id, "invalid_input"),
        };
        let scope = match request.scope.as_str() {
            "user_config" => ConfigScope::UserConfig,
            "project_local_config" if kind == ClientKind::ClaudeCode => {
                ConfigScope::ProjectLocalConfig
            }
            _ => return failure(&request.request_id, "invalid_input"),
        };
        if request.schema_version != 1 || !valid_uuid(&request.request_id) {
            return failure(&request.request_id, "invalid_input");
        }
        let Some(_guard) = PreviewGuard::try_acquire(&self.preview_in_flight) else {
            return failure(&request.request_id, "busy");
        };
        #[allow(unused_mut)]
        let mut snapshot = RetainedSnapshot {
            schema_version: 1,
            request_id: request.request_id.clone(),
            status: "snapshot",
            client_kind: request.client_kind,
            scope: request.scope,
            supported: false,
            review_fingerprint: None,
            active_records: 0,
            selected_records: 0,
            unresolved_records: 0,
            changed_versions: 0,
            completed_archives: 0,
            record_limit: 64,
            archive_limit: 64,
            can_archive: false,
        };
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        {
            let _ = scope;
            if expected.is_some() {
                return failure(&request.request_id, "unavailable");
            }
        }
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        {
            use crate::integrations::transaction_fs::{
                unix_retained_versions as native, TransactionFsError,
            };
            let (roots, selected_scope) = match self.config_roots_for_context(kind) {
                Ok(value) => value,
                Err(_) => return failure(&request.request_id, "scope_changed"),
            };
            if selected_scope != scope {
                return failure(&request.request_id, "scope_changed");
            }
            let config = match read_user_config(kind, &roots) {
                Ok(value) => value,
                Err(_) => return failure(&request.request_id, "unavailable"),
            };
            let path = match roots.target(kind) {
                Ok(ConfigTarget::Single { path, .. }) => path,
                Ok(ConfigTarget::OpenCode { json, jsonc }) => match config.source {
                    ConfigSource::OpenCodeJson | ConfigSource::OpenCodeMissing => json,
                    ConfigSource::OpenCodeJsonc => jsonc,
                    _ => return failure(&request.request_id, "unavailable"),
                },
                Err(_) => return failure(&request.request_id, "unavailable"),
            };
            let Some(store) = &self.ownership_store else {
                return failure(&request.request_id, "unavailable");
            };
            let root = match store.retained_versions_root() {
                Ok(Some(root)) => root,
                Ok(None) if expected.is_none() => {
                    snapshot.supported = true;
                    snapshot.review_fingerprint = Some(config.fingerprint);
                    return RetainedReply::Snapshot(snapshot);
                }
                _ => return failure(&request.request_id, "unavailable"),
            };
            let owner = match kind {
                ClientKind::Codex => "codex.json",
                ClientKind::ClaudeCode => "claude_code.json",
                ClientKind::OpenCodeV1 => "opencode_v1.json",
            };
            let result = match expected.as_deref() {
                Some(expected) => native::archive(&root, &path, owner, expected),
                None => native::inspect(&root, &path, owner),
            };
            let review = match result {
                Ok(value) => value,
                Err(error) => {
                    return failure(
                        &request.request_id,
                        match error {
                            TransactionFsError::Conflict => "conflict",
                            TransactionFsError::RetainedVersionsLimit => "archive_limit",
                            TransactionFsError::CommitFailed
                            | TransactionFsError::RollbackFailed => "commit_unknown",
                            _ => "unavailable",
                        },
                    )
                }
            };
            snapshot.status = if expected.is_some() {
                "archived"
            } else {
                "snapshot"
            };
            snapshot.supported = true;
            snapshot.review_fingerprint = Some(review.review_fingerprint);
            snapshot.active_records = review.active_records;
            snapshot.selected_records = review.selected_records;
            snapshot.unresolved_records = review.unresolved_records;
            snapshot.changed_versions = review.changed_versions;
            snapshot.completed_archives = review.completed_archives;
            snapshot.can_archive = review.can_archive;
            if expected.is_some() {
                self.leases
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .clear();
            }
        }
        RetainedReply::Snapshot(snapshot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manager() -> PlanManager {
        PlanManager::with_test_sources(
            DiscoveryManager::with_choice_store(None),
            ConfigRootInputs::default(),
            Err(StableEndpointAccessError::Unavailable),
        )
    }

    fn request() -> RetainedRequest {
        RetainedRequest {
            schema_version: 1,
            request_id: Uuid::new_v4().to_string(),
            client_kind: "codex".into(),
            scope: "user_config".into(),
        }
    }

    #[test]
    fn retained_protocol_rejects_invalid_scope_fingerprint_and_busy_without_io() {
        let manager = manager();
        let mut bad = request();
        bad.scope = "project_local_config".into();
        let result = serde_json::to_value(manager.retained_versions(bad)).unwrap();
        assert_eq!(result["error"]["code"], "invalid_input");
        let result =
            serde_json::to_value(manager.archive_retained_versions(ArchiveRetainedRequest {
                schema_version: 1,
                request_id: Uuid::new_v4().to_string(),
                client_kind: "codex".into(),
                scope: "user_config".into(),
                expected_review_fingerprint: "secret-like untrusted text".into(),
            }))
            .unwrap();
        assert_eq!(result["error"]["code"], "invalid_input");
        assert!(!result.to_string().contains("secret-like"));
        let _guard = PreviewGuard::try_acquire(&manager.preview_in_flight).unwrap();
        assert_eq!(
            serde_json::to_value(manager.retained_versions(request())).unwrap()["error"]["code"],
            "busy"
        );
    }

    #[test]
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    fn retained_review_is_explicitly_unsupported_without_the_unix_backend() {
        let result = serde_json::to_value(manager().retained_versions(request())).unwrap();
        assert_eq!(result["status"], "snapshot");
        assert_eq!(result["supported"], false);
        assert!(result["review_fingerprint"].is_null());
        assert_eq!(result["active_records"], 0);
        assert_eq!(result["can_archive"], false);
        let result =
            serde_json::to_value(manager().archive_retained_versions(ArchiveRetainedRequest {
                schema_version: 1,
                request_id: Uuid::new_v4().to_string(),
                client_kind: "codex".into(),
                scope: "user_config".into(),
                expected_review_fingerprint: "0".repeat(64),
            }))
            .unwrap();
        assert_eq!(result["error"]["code"], "unavailable");
    }
}
