use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum Preview {
    Ready {
        source: Source,
        suggestions: Vec<Suggestion>,
    },
    Cancelled,
    Busy,
    Unavailable,
    Unreadable,
    InvalidFile,
    TooLarge,
    TimedOut,
    Failed,
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

#[cfg(windows)]
pub(crate) use windows::select;

#[cfg(windows)]
mod windows {
    use super::super::manager::{OwnedJob, RuntimeManager};
    use super::Preview;
    use std::{
        fs::{File, OpenOptions},
        io::Read,
        os::windows::{fs::MetadataExt, fs::OpenOptionsExt, process::CommandExt},
        path::Path,
        process::{Command, Stdio},
        sync::atomic::{AtomicBool, Ordering},
        thread,
        time::{Duration, Instant},
    };
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_OPEN_REPARSE_POINT,
    };

    const MAX_BYTES: u64 = 16 * 1024 * 1024;
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
                analyze(manager, &path).unwrap_or_else(|failure| failure)
            }
            Some(Ok(Some(_))) => Preview::Unavailable,
            Some(Ok(None)) => Preview::Cancelled,
            _ => Preview::Failed,
        };
        if allowed() {
            reply
        } else {
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
        if metadata.len() > MAX_BYTES {
            return Err(Preview::TooLarge);
        }
        Ok(file)
    }

    pub(super) fn analyze(manager: &RuntimeManager, path: &Path) -> Result<Preview, Preview> {
        let file = open_session(path)?;
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
            ])
            .current_dir(root)
            .env_clear()
            .env("SystemRoot", system_root)
            .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
            .stdin(file)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| Preview::Failed)?;
        if job.assign(&child).is_err() {
            let _ = child.kill();
            let _ = child.wait();
            return Err(Preview::Failed);
        }
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            match child.try_wait() {
                Ok(Some(status)) if status.success() => break,
                Ok(Some(_)) | Err(_) => return Err(Preview::Failed),
                Ok(None) if Instant::now() >= deadline => {
                    drop(job);
                    let _ = child.wait();
                    return Err(Preview::TimedOut);
                }
                Ok(None) => thread::sleep(Duration::from_millis(20)),
            }
        }
        drop(job);
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
            reply @ (Preview::Ready { .. } | Preview::InvalidFile | Preview::TooLarge) => Ok(reply),
            _ => Err(Preview::Failed),
        }
    }
}
