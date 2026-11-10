use super::*;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

const STABLE: &[u8] = include_bytes!("../../../tests/fixtures/update/stable.json");
const STABLE_SIGNATURE: &[u8] = include_bytes!("../../../tests/fixtures/update/stable.json.sig");
const PREVIEW: &[u8] = include_bytes!("../../../tests/fixtures/update/preview.json");
const PREVIEW_SIGNATURE: &[u8] = include_bytes!("../../../tests/fixtures/update/preview.json.sig");
const INSTALLER: &[u8] = b"headroom-h042-fixture-installer-v0.1.1\n";

#[derive(Clone, Copy)]
enum Mode {
    Normal,
    InvalidSignature,
    Redirect,
    WrongHash,
    ShortBody,
}

struct Feed {
    base: String,
    mode: Arc<Mutex<Mode>>,
    hits: Arc<Mutex<Vec<String>>>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl Feed {
    fn start(mode: Mode) -> Self {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        listener.set_nonblocking(true).unwrap();
        let base = format!("http://{}/releases/", listener.local_addr().unwrap());
        let mode = Arc::new(Mutex::new(mode));
        let hits = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let (worker_mode, worker_hits, worker_stop) = (mode.clone(), hits.clone(), stop.clone());
        let worker = thread::spawn(move || {
            while !worker_stop.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => serve(stream, &worker_mode, &worker_hits),
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5))
                    }
                    Err(error) => panic!("fixture accept failed: {:?}", error.kind()),
                }
            }
        });
        Self {
            base,
            mode,
            hits,
            stop,
            worker: Some(worker),
        }
    }

    fn flow(&self, app_data: Option<PathBuf>) -> UpdateFlow {
        UpdateFlow::with_source(
            "dev.headroom.desktop".into(),
            semver::Version::new(0, 1, 0),
            app_data,
            ReleaseSource::fixture(&self.base),
        )
    }
}

impl Drop for Feed {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.worker.take().unwrap().join().unwrap();
    }
}

use std::io;

fn serve(mut stream: TcpStream, mode: &Mutex<Mode>, hits: &Mutex<Vec<String>>) {
    stream.set_nonblocking(false).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let mut reader = BufReader::new(&stream);
    let mut line = String::new();
    if reader.read_line(&mut line).unwrap_or_default() == 0 {
        return;
    }
    let path = line
        .split_ascii_whitespace()
        .nth(1)
        .unwrap_or("")
        .to_owned();
    for _ in 0..24 {
        line.clear();
        if reader.read_line(&mut line).unwrap_or_default() == 0 || line == "\r\n" {
            break;
        }
    }
    hits.lock().unwrap().push(path.clone());
    let mode = *mode.lock().unwrap();
    if matches!(mode, Mode::Redirect) {
        let _ = stream.write_all(b"HTTP/1.1 302 Found\r\nLocation: /redirect-target\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
        return;
    }
    let data: &[u8] = match path.as_str() {
        "/releases/stable/manifest.json" => STABLE,
        "/releases/preview/manifest.json" => PREVIEW,
        "/releases/stable/manifest.json.sig" if matches!(mode, Mode::InvalidSignature) => {
            b"invalid signature"
        }
        "/releases/stable/manifest.json.sig" => STABLE_SIGNATURE,
        "/releases/preview/manifest.json.sig" => PREVIEW_SIGNATURE,
        "/releases/stable/Headroom_0.1.1_x64-setup.exe"
        | "/releases/preview/Headroom_0.1.1-rc.1_x64-setup.exe" => INSTALLER,
        _ => {
            let _ = stream.write_all(
                b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            );
            return;
        }
    };
    let _ = write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        data.len()
    );
    if data == INSTALLER {
        match mode {
            Mode::WrongHash => {
                let _ = stream.write_all(b"!");
                let _ = stream.write_all(&data[1..]);
                return;
            }
            Mode::ShortBody => {
                let _ = stream.write_all(&data[..data.len() - 1]);
                return;
            }
            _ => (),
        }
    }
    let _ = stream.write_all(data);
}

fn request(flow: &UpdateFlow, action: Action, engine_stopped: bool) -> Reply {
    flow.handle(
        Request {
            request_id: Uuid::new_v4().to_string(),
            action,
        },
        || engine_stopped,
    )
}

fn test_root() -> PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("work/proofs/update/flow");
    let path = root.join(Uuid::new_v4().to_string());
    fs::create_dir_all(&path).unwrap();
    path
}

fn check(flow: &UpdateFlow, channel: UpdateChannel) -> CandidateView {
    let reply = request(flow, Action::Check { channel }, true);
    assert_eq!(reply.status, Status::Available, "{:?}", reply.error);
    assert_eq!(reply.channel, Some(channel));
    reply.candidate.unwrap()
}

#[test]
fn verifies_a_download_on_disk_and_waits_for_the_engine() {
    let feed = Feed::start(Mode::Normal);
    let root = test_root().join("app-data");
    let flow = feed.flow(Some(root.clone()));
    assert_eq!(request(&flow, Action::Snapshot, true).status, Status::Idle);
    assert!(feed.hits.lock().unwrap().is_empty());
    let candidate = check(&flow, UpdateChannel::Stable);
    assert_eq!(candidate.version, "0.1.1");
    assert_eq!(candidate.notes, "Yerel imza örneği.");
    assert_eq!(candidate.size_bytes, INSTALLER.len() as u64);
    let reply = request(
        &flow,
        Action::Download {
            candidate_id: candidate.id,
        },
        false,
    );
    assert_eq!(reply.status, Status::WaitingForEngine, "{:?}", reply.error);
    assert_eq!(
        request(&flow, Action::Snapshot, false).status,
        Status::WaitingForEngine
    );
    assert_eq!(request(&flow, Action::Snapshot, true).status, Status::Ready);
    assert_eq!(feed.hits.lock().unwrap().len(), 3);
    let files: Vec<_> = fs::read_dir(&root)
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(files.len(), 1);
    assert!(files[0].file_name().to_string_lossy().ends_with(".part"));
    assert!(fs::OpenOptions::new()
        .write(true)
        .open(files[0].path())
        .is_err());
    drop(flow);
    assert_eq!(fs::read_dir(root).unwrap().count(), 0);
}

#[test]
fn rejects_redirects_and_untrusted_metadata_without_a_candidate() {
    for (mode, expected, hits) in [
        (Mode::InvalidSignature, Failure::InvalidRelease, 2),
        (Mode::Redirect, Failure::SourceFailed, 1),
    ] {
        let feed = Feed::start(mode);
        let flow = feed.flow(None);
        let reply = request(
            &flow,
            Action::Check {
                channel: UpdateChannel::Stable,
            },
            true,
        );
        assert_eq!(reply.error, Some(expected));
        assert!(reply.candidate.is_none());
        assert_eq!(request(&flow, Action::Snapshot, true).status, Status::Idle);
        assert_eq!(feed.hits.lock().unwrap().len(), hits);
    }
}

#[test]
fn drops_corrupt_or_interrupted_staging_and_allows_retry() {
    for (mode, expected) in [
        (Mode::WrongHash, Failure::IntegrityFailed),
        (Mode::ShortBody, Failure::DownloadFailed),
    ] {
        let feed = Feed::start(mode);
        let root = test_root().join("app-data");
        let flow = feed.flow(Some(root.clone()));
        let candidate = check(&flow, UpdateChannel::Stable);
        let download = || Action::Download {
            candidate_id: candidate.id.clone(),
        };
        assert_eq!(request(&flow, download(), true).error, Some(expected));
        assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
        assert_eq!(
            request(&flow, Action::Snapshot, true).status,
            Status::Available
        );
        *feed.mode.lock().unwrap() = Mode::Normal;
        assert_eq!(request(&flow, download(), true).status, Status::Ready);
    }
}

#[test]
fn channel_check_invalidates_the_prior_download_id() {
    let feed = Feed::start(Mode::Normal);
    let flow = feed.flow(Some(test_root().join("app-data")));
    let stable = check(&flow, UpdateChannel::Stable);
    let preview = check(&flow, UpdateChannel::Preview);
    assert_eq!(preview.version, "0.1.1-rc.1");
    assert_ne!(preview.id, stable.id);
    assert_eq!(
        request(
            &flow,
            Action::Download {
                candidate_id: stable.id
            },
            true
        )
        .error,
        Some(Failure::CandidateChanged)
    );
    assert_eq!(feed.hits.lock().unwrap().len(), 4);
    assert_eq!(
        request(
            &flow,
            Action::Download {
                candidate_id: preview.id
            },
            true
        )
        .status,
        Status::Ready
    );
    assert_eq!(
        feed.hits.lock().unwrap().last().unwrap(),
        "/releases/preview/Headroom_0.1.1-rc.1_x64-setup.exe"
    );
}

#[test]
fn configures_only_a_credential_free_https_source_or_explicit_loopback_fixture() {
    for base in [
        "http://updates.example/",
        "https://u@updates.example/",
        "https://updates.example/?secret=fixture",
        "https://updates.example/#fragment",
        "https://updates.example/file",
        "https://127.0.0.1/",
        "https://localhost./",
    ] {
        assert!(
            ReleaseSource::new(base, "fixture-public-key", false).is_none(),
            "{base}"
        );
    }
    assert!(ReleaseSource::new(
        "https://updates.example/windows/",
        "fixture-public-key",
        false
    )
    .is_some());
    assert!(ReleaseSource::fixture("http://localhost:12345/").is_none());
    assert!(ReleaseSource::fixture("http://127.0.0.1:12345/").is_some());
    let flow = UpdateFlow::with_source(
        "dev.headroom.desktop".into(),
        semver::Version::new(0, 1, 0),
        None,
        None,
    );
    assert_eq!(
        request(
            &flow,
            Action::Check {
                channel: UpdateChannel::Stable
            },
            true
        )
        .status,
        Status::Unavailable
    );
}

#[test]
#[ignore = "requires the isolated H042 signed NSIS fixture feed"]
fn prepares_the_signed_nsis_application_and_refuses_incompatible_output() {
    let base = std::env::var("HEADROOM_H042_PACKAGE_FIXTURE_URL").unwrap();
    let key =
        fs::read_to_string(std::env::var_os("HEADROOM_H042_PACKAGE_FIXTURE_PUBLIC_KEY").unwrap())
            .unwrap();
    assert!(key.len() <= 1024);
    let key = Box::leak(key.into_boxed_str());
    let root = test_root().join("app-data");
    let flow = |base: &str| {
        UpdateFlow::with_source(
            "dev.headroom.desktop.h038-qa".into(),
            semver::Version::new(0, 0, 9),
            Some(root.clone()),
            Some(ReleaseSource::new(base, key, true).unwrap()),
        )
    };
    let update = flow(&base);
    let candidate = check(&update, UpdateChannel::Stable);
    let reply = request(
        &update,
        Action::Download {
            candidate_id: candidate.id,
        },
        false,
    );
    assert_eq!(reply.status, Status::WaitingForEngine, "{:?}", reply.error);
    {
        let mut phase = update.0.phase.lock().unwrap();
        let Phase::Candidate(candidate) = &mut *phase else {
            panic!("staged candidate missing")
        };
        candidate
            .update
            .verify_application(candidate.staged.as_mut().unwrap())
            .unwrap();
    }
    assert_eq!(
        request(&update, Action::Snapshot, true).status,
        Status::Ready
    );
    assert_eq!(fs::read_dir(&root).unwrap().count(), 1);

    let changed = check(&update, UpdateChannel::Preview);
    assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
    let reply = request(
        &update,
        Action::Download {
            candidate_id: changed.id,
        },
        true,
    );
    assert_eq!(reply.error, Some(Failure::IntegrityFailed));
    assert_eq!(
        request(&update, Action::Snapshot, true).status,
        Status::Available
    );
    assert_eq!(fs::read_dir(&root).unwrap().count(), 0);

    let mismatch = flow(&format!("{base}state-format/"));
    let reply = request(
        &mismatch,
        Action::Check {
            channel: UpdateChannel::Stable,
        },
        true,
    );
    assert_eq!(reply.error, Some(Failure::InvalidRelease));
    assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
}
