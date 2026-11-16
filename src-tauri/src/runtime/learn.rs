use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const MAX_BYTES: usize = 16 * 1024 * 1024;
const REPLAY_TTL: Duration = Duration::from_secs(15 * 60);

#[derive(Clone, Default)]
pub(crate) struct ReplayStore(Arc<Mutex<Option<Capture>>>);

#[derive(Clone)]
struct Capture {
    id: String,
    owner: usize,
    created: Instant,
    raw: Arc<Vec<u8>>,
}

impl ReplayStore {
    fn keep(&self, owner: usize, raw: Arc<Vec<u8>>) -> Option<String> {
        if owner == 0 || raw.len() > MAX_BYTES {
            return None;
        }
        let id = uuid::Uuid::new_v4().to_string();
        *self.0.lock().ok()? = Some(Capture {
            id: id.clone(),
            owner,
            created: Instant::now(),
            raw,
        });
        Some(id)
    }

    fn original(&self, owner: usize, id: &str) -> Option<Capture> {
        let mut entry = self.0.lock().ok()?;
        if entry
            .as_ref()
            .is_some_and(|capture| capture.created.elapsed() >= REPLAY_TTL)
        {
            *entry = None;
        }
        entry
            .as_ref()
            .filter(|capture| capture.owner == owner && capture.id == id)
            .cloned()
    }

    pub(crate) fn release(&self, owner: usize, id: &str) {
        if let Ok(mut entry) = self.0.lock() {
            if entry
                .as_ref()
                .is_some_and(|capture| capture.owner == owner && capture.id == id)
            {
                *entry = None;
            }
        }
    }
}

#[derive(Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum Preview {
    Ready {
        source: Source,
        suggestions: Vec<Suggestion>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        replay_id: Option<String>,
    },
    TrialReady {
        report: ReplayReport,
    },
    Cancelled,
    Busy,
    Unavailable,
    Unreadable,
    InvalidFile,
    TooLarge,
    TimedOut,
    Failed,
    Expired,
}

#[derive(Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReplayReport {
    rule: Rule,
    checked: u32,
    protected: u32,
    ambiguous: u32,
    eligible: u32,
    compacted: u32,
    before_bytes: u64,
    after_bytes: u64,
    round_trip_exact: bool,
    #[serde(skip_serializing)]
    original_sha256: String,
}

#[derive(Debug, Deserialize, PartialEq, Eq, Serialize)]
enum Rule {
    #[serde(rename = "repeated_log_lines_v1")]
    RepeatedLogLinesV1,
}

#[derive(Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Source {
    Codex,
    ClaudeCode,
}

#[derive(Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Suggestion {
    kind: SuggestionKind,
    failures: u32,
    recoveries: u32,
    first_line: u32,
}

#[derive(Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum SuggestionKind {
    Path,
    Dependency,
    Permission,
    Timeout,
    Failure,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replay_archive_preserves_bytes_and_rejects_wrong_scope_replacement_and_expiry() {
        let store = ReplayStore::default();
        let raw = Arc::new(b"\xef\xbb\xbf{\"stdout\":\"private\",\"exit_code\":1}\r\n".to_vec());
        let id = store.keep(17, Arc::clone(&raw)).unwrap();
        assert_eq!(&*store.original(17, &id).unwrap().raw, &*raw);
        assert!(store.original(18, &id).is_none());
        store.release(18, &id);
        assert!(store.original(17, &id).is_some());
        assert!(store.keep(17, Arc::new(vec![0; MAX_BYTES + 1])).is_none());
        assert!(store.original(17, &id).is_some());
        let next = store.keep(17, Arc::clone(&raw)).unwrap();
        store.release(17, &id);
        assert!(store.original(17, &id).is_none());
        assert!(store.original(17, &next).is_some());
        store.0.lock().unwrap().as_mut().unwrap().created = Instant::now() - REPLAY_TTL;
        assert!(store.original(17, &next).is_none());
        assert!(store.0.lock().unwrap().is_none());
        assert!(ReplayStore::default().original(17, &id).is_none());
        let last = store.keep(17, raw).unwrap();
        store.release(17, &last);
        assert!(store.original(17, &last).is_none());
    }
}

#[cfg(windows)]
pub(crate) use windows::{replay, select};

#[cfg(windows)]
mod windows {
    use super::super::manager::{OwnedJob, RuntimeManager};
    use super::{Preview, ReplayStore, MAX_BYTES};
    use sha2::{Digest, Sha256};
    use std::{
        fs::{File, OpenOptions},
        io::{Read, Write},
        os::windows::{fs::MetadataExt, fs::OpenOptionsExt, process::CommandExt},
        path::Path,
        process::{Command, Stdio},
        sync::atomic::{AtomicBool, Ordering},
        sync::Arc,
        thread,
        time::{Duration, Instant},
    };
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_OPEN_REPARSE_POINT,
    };

    const MAX_REPLY: u64 = 2048;
    static SELECTING: AtomicBool = AtomicBool::new(false);

    struct PreviewGuard;

    impl Drop for PreviewGuard {
        fn drop(&mut self) {
            SELECTING.store(false, Ordering::Release);
        }
    }

    pub(crate) fn select(
        manager: &RuntimeManager,
        owner: usize,
        turkish: bool,
        experimental: Option<&crate::state::store::PreferenceStore>,
        archive: &ReplayStore,
    ) -> Preview {
        let allowed = || experimental.is_none_or(|store| store.experimental_rules_enabled());
        if !allowed() {
            return Preview::Unavailable;
        }
        if SELECTING.swap(true, Ordering::AcqRel) {
            return Preview::Busy;
        }
        let _guard = PreviewGuard;
        let picker = thread::Builder::new()
            .name("headroom-learn-picker-sta".into())
            .spawn(move || crate::integrations::picker::select_learn_session(owner, turkish));
        let reply = match picker.ok().and_then(|thread| thread.join().ok()) {
            Some(Ok(Some(path))) if allowed() => {
                let analyzed = read_session(&path).and_then(|raw| {
                    let mut reply = analyze(manager, Arc::clone(&raw), false)?;
                    if experimental.is_some() && allowed() {
                        if let Preview::Ready { replay_id, .. } = &mut reply {
                            *replay_id = Some(archive.keep(owner, raw).ok_or(Preview::Failed)?);
                        }
                    }
                    Ok(reply)
                });
                analyzed.unwrap_or_else(|failure| failure)
            }
            Some(Ok(Some(_))) => Preview::Unavailable,
            Some(Ok(None)) => Preview::Cancelled,
            _ => Preview::Failed,
        };
        if allowed() {
            reply
        } else {
            if let Preview::Ready {
                replay_id: Some(id),
                ..
            } = &reply
            {
                archive.release(owner, id);
            }
            Preview::Unavailable
        }
    }

    fn open_session(path: &Path) -> Result<File, Preview> {
        if !path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("jsonl"))
        {
            return Err(Preview::InvalidFile);
        }
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
            .open(path)
            .map_err(|_| Preview::Unreadable)?;
        let metadata = file.metadata().map_err(|_| Preview::Unreadable)?;
        if !metadata.is_file() || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(Preview::Unreadable);
        }
        if metadata.len() > MAX_BYTES as u64 {
            return Err(Preview::TooLarge);
        }
        Ok(file)
    }

    fn read_session(path: &Path) -> Result<Arc<Vec<u8>>, Preview> {
        let file = open_session(path)?;
        let mut raw = Vec::new();
        file.take(MAX_BYTES as u64 + 1)
            .read_to_end(&mut raw)
            .map_err(|_| Preview::Unreadable)?;
        if raw.len() > MAX_BYTES {
            return Err(Preview::TooLarge);
        }
        Ok(Arc::new(raw))
    }

    pub(crate) fn replay(
        manager: &RuntimeManager,
        owner: usize,
        id: &str,
        preferences: &crate::state::store::PreferenceStore,
        archive: &ReplayStore,
    ) -> Preview {
        if !preferences.experimental_rules_enabled() {
            return Preview::Unavailable;
        }
        let Some(capture) = archive.original(owner, id) else {
            return Preview::Expired;
        };
        if SELECTING.swap(true, Ordering::AcqRel) {
            return Preview::Busy;
        }
        let _guard = PreviewGuard;
        let reply =
            analyze(manager, Arc::clone(&capture.raw), true).unwrap_or_else(|failure| failure);
        if !preferences.experimental_rules_enabled() {
            return Preview::Unavailable;
        }
        if archive.original(owner, id).is_none() {
            return Preview::Expired;
        }
        match reply {
            Preview::TrialReady { ref report }
                if report.original_sha256 == format!("{:x}", Sha256::digest(&*capture.raw))
                    && report.round_trip_exact
                    && report.checked <= 64_000
                    && report.compacted <= report.eligible
                    && report.after_bytes <= report.before_bytes
                    && report.before_bytes <= MAX_BYTES as u64
                    && report.protected as u64
                        + report.eligible as u64
                        + report.ambiguous as u64
                        == report.checked as u64 =>
            {
                reply
            }
            Preview::InvalidFile | Preview::TooLarge | Preview::TimedOut => reply,
            _ => Preview::Failed,
        }
    }

    fn analyze(
        manager: &RuntimeManager,
        raw: Arc<Vec<u8>>,
        replay: bool,
    ) -> Result<Preview, Preview> {
        let (root, python) = manager.learn_payload().ok_or(Preview::Unavailable)?;
        let system_root = std::env::var_os("SystemRoot").ok_or(Preview::Unavailable)?;
        let job = OwnedJob::create().map_err(|_| Preview::Failed)?;
        let mut child = Command::new(python)
            .args([
                "-I",
                "-S",
                "-B",
                "-c",
                include_str!("../../../runtime/learn_preview.py"),
                if replay { "replay" } else { "preview" },
            ])
            .current_dir(root)
            .env_clear()
            .env("SystemRoot", system_root)
            .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| Preview::Failed)?;
        if job.assign(&child).is_err() {
            let _ = child.kill();
            let _ = child.wait();
            return Err(Preview::Failed);
        }
        let mut stdin = child.stdin.take().ok_or(Preview::Failed)?;
        let writer = thread::Builder::new()
            .name("headroom-learn-input".into())
            .spawn(move || stdin.write_all(&raw))
            .map_err(|_| Preview::Failed)?;
        let deadline = Instant::now() + Duration::from_secs(15);
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) if status.success() => break Ok(()),
                Ok(Some(_)) | Err(_) => break Err(Preview::Failed),
                Ok(None) if Instant::now() >= deadline => {
                    break Err(Preview::TimedOut);
                }
                Ok(None) => thread::sleep(Duration::from_millis(20)),
            }
        };
        drop(job);
        if status.is_err() {
            let _ = child.wait();
        }
        let written = writer.join().map_err(|_| Preview::Failed)?;
        status?;
        written.map_err(|_| Preview::Failed)?;
        let mut bytes = Vec::new();
        child
            .stdout
            .take()
            .ok_or(Preview::Failed)?
            .take(MAX_REPLY + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| Preview::Failed)?;
        if bytes.len() as u64 > MAX_REPLY {
            return Err(Preview::Failed);
        }
        match serde_json::from_slice(&bytes).map_err(|_| Preview::Failed)? {
            reply @ (Preview::Ready { .. }
            | Preview::TrialReady { .. }
            | Preview::InvalidFile
            | Preview::TooLarge) => Ok(reply),
            _ => Err(Preview::Failed),
        }
    }
}
