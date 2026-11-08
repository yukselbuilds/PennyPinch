//! Passive inspection of owned fields; no lease, picker, commit, or auth read.
use super::*;
use crate::integrations::transaction::{undo_owned_endpoint, TransactionError};
use crate::integrations::transaction_fs::FileTransaction;
use crate::state::ownership::{config_path_binding, OwnershipState};
use sha2::{Digest, Sha256};

pub(crate) struct ConfigObservation {
    pub state: &'static str,
    pub scope: &'static str,
    pub endpoint: Option<(String, u16)>,
    pub context_fingerprint: Option<String>,
}
impl ConfigObservation {
    fn unknown(state: &'static str) -> Self {
        Self {
            state,
            scope: "unknown",
            endpoint: None,
            context_fingerprint: None,
        }
    }
}

impl PlanManager {
    pub(crate) fn recovery_config(
        &self,
        kind: ClientKind,
        installation: Option<&InstallationSummary>,
        scan_generation: u64,
    ) -> ConfigObservation {
        let Some(_guard) = PreviewGuard::try_acquire(&self.preview_in_flight) else {
            return ConfigObservation::unknown("busy");
        };
        let inspect = || -> Result<ConfigObservation, ()> {
            let store = self.ownership_store.as_ref().ok_or(())?;
            let mut tx = FileTransaction::begin().map_err(|_| ())?;
            let snapshot = store
                .read_snapshot_in_transaction(&mut tx, kind)
                .map_err(|_| ())?;
            let result = match snapshot {
                None => ConfigObservation::unknown("not_owned"),
                Some(snapshot) if snapshot.record.state() != OwnershipState::Active => {
                    ConfigObservation::unknown("not_owned")
                }
                Some(snapshot) => {
                    let record = snapshot.record;
                    let scope = match record.config_scope().ok_or(())? {
                        ConfigScope::UserConfig => "user_config",
                        ConfigScope::ProjectLocalConfig => "project_local_config",
                    };
                    let mut result = ConfigObservation {
                        state: "not_rechecked",
                        scope,
                        endpoint: Some((record.endpoint_id().into(), record.endpoint_port())),
                        context_fingerprint: None,
                    };
                    if let Some(installation) = installation {
                        if let Ok((state, fingerprint)) =
                            self.inspect_owned_config(&record, installation, scan_generation)
                        {
                            result.state = state;
                            result.context_fingerprint = fingerprint;
                        } else {
                            result.state = "unavailable";
                        }
                    }
                    result
                }
            };
            tx.rollback().map_err(|_| ())?;
            Ok(result)
        };
        inspect().unwrap_or_else(|_| ConfigObservation::unknown("unavailable"))
    }

    fn inspect_owned_config(
        &self,
        record: &crate::state::ownership::OwnershipRecord,
        installation: &InstallationSummary,
        generation: u64,
    ) -> Result<(&'static str, Option<String>), ()> {
        let kind = record.client_kind().ok_or(())?;
        let (roots, scope) = self.config_roots_for_context(kind).map_err(|_| ())?;
        if record.config_scope() != Some(scope) {
            return Ok(("not_rechecked", None));
        }
        let first = read_user_config(kind, &roots).map_err(|_| ())?;
        if first.presence == ConfigPresence::Missing || record.config_source() != Some(first.source)
        {
            return Ok(("conflict", None));
        }
        let path = match roots.target(kind).map_err(|_| ())? {
            ConfigTarget::Single { path, .. } => path,
            ConfigTarget::OpenCode { json, jsonc } => match first.source {
                ConfigSource::OpenCodeJson => json,
                ConfigSource::OpenCodeJsonc => jsonc,
                _ => return Err(()),
            },
        };
        if config_path_binding(&path).map_err(|_| ())? != record.config_path_binding_sha256() {
            return Ok(("not_rechecked", None));
        }
        // The pure undo validator checks all owned fields, including OpenCode routes.
        // Its returned bytes are discarded; no rollback or repair is written.
        let fields_match =
            match undo_owned_endpoint(&record.patch_receipt().map_err(|_| ())?, &first.bytes) {
                Ok(_) => true,
                Err(TransactionError::Conflict | TransactionError::InvalidConfig) => false,
                Err(_) => return Err(()),
            };
        let context = PlanContext {
            request_id: Uuid::new_v4().to_string(),
            installation_id: installation.installation_id.clone(),
            client_kind: kind,
            config_source: first.source,
            surface: installation.surface.into(),
            version: installation.version.clone(),
            scope,
            scan_generation: generation,
            endpoint: super::super::plan::StableEndpoint {
                endpoint_id: record.endpoint_id().into(),
                host: "127.0.0.1".into(),
                port: record.endpoint_port(),
            },
            now_unix_seconds: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|_| ())?
                .as_secs(),
        };
        let plan =
            preview_config_with_presence(&context, &first.bytes, first.presence).map_err(|_| ())?;
        let layers = (kind == ClientKind::ClaudeCode)
            .then(|| claude_layers::observe(&roots, scope, Some(&first), &context))
            .transpose()
            .map_err(|_| ())?;
        let environment =
            (kind == ClientKind::OpenCodeV1).then(|| (self.opencode_environment_provider)());
        let second = read_user_config(kind, &roots).map_err(|_| ())?;
        let second_layers = (kind == ClientKind::ClaudeCode)
            .then(|| claude_layers::observe(&roots, scope, Some(&second), &context))
            .transpose()
            .map_err(|_| ())?;
        let second_environment =
            (kind == ClientKind::OpenCodeV1).then(|| (self.opencode_environment_provider)());
        let (current_roots, current_scope) = self.config_roots_for_context(kind).map_err(|_| ())?;
        if first.fingerprint != second.fingerprint
            || first.source != second.source
            || first.presence != second.presence
            || current_scope != scope
            || !same_config_target(&roots, &current_roots, kind)
            || layers.as_ref().map(|l| &l.fingerprint)
                != second_layers.as_ref().map(|l| &l.fingerprint)
            || environment.as_ref().map(|e| &e.fingerprint)
                != second_environment.as_ref().map(|e| &e.fingerprint)
        {
            return Err(());
        }
        let mut digest = Sha256::new();
        digest.update(b"headroom-compatibility-config/v1\0");
        for value in [
            Some(first.fingerprint.as_str()),
            layers.as_ref().map(|l| l.fingerprint.as_str()),
            environment.as_ref().map(|e| e.fingerprint.as_str()),
        ] {
            digest.update(value.unwrap_or("none").as_bytes());
            digest.update([0]);
        }
        Ok((
            if fields_match
                && plan.blockers.is_empty()
                && layers.as_ref().is_none_or(|l| l.route_blockers.is_empty())
                && environment.as_ref().is_none_or(|e| !e.has_override)
            {
                "matches_owned_endpoint"
            } else {
                "conflict"
            },
            Some(format!("{:x}", digest.finalize())),
        ))
    }
}
