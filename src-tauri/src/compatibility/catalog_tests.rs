use super::*;
use crate::integrations::{config_io::PlanManager, discovery::DiscoveryManager};
use crate::runtime::manager::RuntimeManager;
use std::{collections::HashSet, path::PathBuf};

fn source() -> serde_json::Value {
    serde_json::from_str(include_str!("../../../compatibility/clients.json")).unwrap()
}
fn request() -> Request {
    Request {
        schema_version: 1,
        request_id: uuid::Uuid::new_v4().to_string(),
    }
}

#[test]
fn compatibility_catalog_embedded_records_have_scoped_existing_evidence() {
    let project = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf();
    let catalog = parse(&source().to_string()).unwrap();
    let records = source()["records"].as_array().unwrap().clone();
    assert_eq!(catalog.records.len(), records.len());
    let mut ids = HashSet::new();
    for row in records {
        assert!(ids.insert(row["id"].as_str().unwrap().to_owned()));
        for evidence in row["evidence"].as_array().unwrap() {
            let path = project.join(evidence.as_str().unwrap());
            assert!(path.is_file(), "missing scoped evidence");
        }
        for capability in row["capabilities"].as_object().unwrap().values() {
            assert!(project
                .join(capability["evidence"].as_str().unwrap())
                .is_file());
        }
        if row["status"] == "live_verified" {
            let evidence: Vec<_> = row["evidence"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| std::fs::read_to_string(project.join(p.as_str().unwrap())).unwrap())
                .collect();
            for version in row["match"]["versions"].as_array().unwrap() {
                assert!(
                    evidence
                        .iter()
                        .any(|text| text.contains(version.as_str().unwrap())),
                    "live version lacks cited evidence"
                );
            }
        }
    }
}

#[test]
fn compatibility_release_versions_match_the_application_and_runtime_sources() {
    let catalog = parse(&source().to_string()).unwrap();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf();
    for relative in ["package.json", "src-tauri/tauri.conf.json"] {
        let value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(root.join(relative)).unwrap()).unwrap();
        assert_eq!(value["version"], catalog.release.app_version);
    }
    let manifest: serde_json::Value =
        serde_json::from_str(include_str!("../../../runtime/manifest.json")).unwrap();
    assert_eq!(
        manifest["engine"]["version"],
        catalog.release.engine_version
    );
    assert_eq!(manifest["engine"]["python_version"], "3.13.13");
    assert_eq!(
        source()["product_capabilities"][0]["status"],
        "local_fixture_pass"
    );
    assert_eq!(source()["environment_gates"][0]["status"], "not_tested");
}

#[test]
fn compatibility_catalog_rejects_unscoped_live_status_duplicates_and_bad_paths() {
    for change in 0..6 {
        let mut value = source();
        match change {
            0 => value["records"][0]["status"] = serde_json::json!("verified_everywhere"),
            1 => value["records"][0]["evidence"] = serde_json::json!([]),
            2 => value["records"][1]["id"] = value["records"][0]["id"].clone(),
            3 => {
                value["records"][0]["evidence"] = serde_json::json!(["docs/proofs/../../secret.md"])
            }
            4 => value["records"][0]["status"] = serde_json::json!("live_verified"),
            _ => value["release"]["channel"] = serde_json::json!("stable"),
        }
        assert!(parse(&value.to_string()).is_err());
    }
}

#[test]
fn compatibility_snapshot_is_read_only_and_never_claims_an_installed_auth_session() {
    let runtime = RuntimeManager::new(None, None);
    let discovery = DiscoveryManager::new();
    let plans = PlanManager::new(discovery.clone(), runtime.clone());
    let manager = CatalogManager::default();
    let req = request();
    let id = req.request_id.clone();
    let before =
        serde_json::to_value(runtime.get_runtime_snapshot(uuid::Uuid::new_v4().to_string()))
            .unwrap();
    let value = serde_json::to_value(manager.read(req, &runtime, &discovery, &plans)).unwrap();
    assert_eq!(value["request_id"], id);
    assert_eq!(value["status"], "snapshot");
    assert_eq!(value["automatic_updates"], false);
    assert_eq!(value["updater_available"], false);
    assert_eq!(value["channel"], "development");
    assert_eq!(value["engine"]["pinned_version"], "0.41.0");
    assert!(value["scan_generation"].is_null());
    assert!(value["context_fingerprint"].is_null());
    assert!(value["installations"].as_array().unwrap().is_empty());
    assert_eq!(value["adapters"].as_array().unwrap().len(), 3);
    let after =
        serde_json::to_value(runtime.get_runtime_snapshot(uuid::Uuid::new_v4().to_string()))
            .unwrap();
    for field in [
        "runtime_generation",
        "engine_version",
        "engine",
        "compression",
    ] {
        assert_eq!(before[field], after[field]);
    }
    assert!(!value.to_string().contains("current_auth"));
    let failed = serde_json::to_value(manager.read(
        Request {
            schema_version: 2,
            request_id: uuid::Uuid::new_v4().to_string(),
        },
        &runtime,
        &discovery,
        &plans,
    ))
    .unwrap();
    assert_eq!(failed["error"]["code"], "invalid_input");
    manager.0.store(true, Ordering::SeqCst);
    let busy = serde_json::to_value(manager.read(request(), &runtime, &discovery, &plans)).unwrap();
    assert_eq!(busy["error"]["code"], "busy");
    manager.0.store(false, Ordering::SeqCst);
}
