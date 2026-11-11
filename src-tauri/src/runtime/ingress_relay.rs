//! Generation-pinned, opaque TCP forwarding for the stable H040 ingress.
//!
//! This module never parses HTTP, retries a backend request, or logs/persists
//! stream content. Each accepted TCP connection receives one immutable target.

use crate::runtime::ingress::StableIngress;
#[cfg(windows)]
use crate::runtime::ingress::{verify_backend_peer, IngressError};
use std::collections::HashMap;
use std::io::{self, Read, Write};
#[cfg(windows)]
use std::net::SocketAddrV4;
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
#[cfg(target_os = "macos")]
use std::path::PathBuf;
#[cfg(windows)]
type BackendStream = TcpStream;
#[cfg(target_os = "macos")]
type BackendStream = std::os::unix::net::UnixStream;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use uuid::Uuid;

const MAX_CONNECTIONS_HARD_LIMIT: usize = 64;
const MAX_BUFFER_BYTES: usize = 64 * 1024;
const MAX_TIMEOUT: Duration = Duration::from_secs(24 * 60 * 60);
const PEER_ROW_VISIBILITY_BUDGET: Duration = Duration::from_millis(40);
const ACCEPT_POLL_INTERVAL: Duration = Duration::from_millis(5);
const ACCEPT_JOIN_RESERVE: Duration = Duration::from_millis(50);

/// An internal lease for one pinned backend generation.
#[derive(Clone)]
pub struct BackendTarget {
    #[cfg(windows)]
    address: SocketAddrV4,
    #[cfg(target_os = "macos")]
    socket_path: PathBuf,
    generation: Uuid,
    expected_engine_pid: u32,
    liveness: Arc<dyn Fn() -> bool + Send + Sync>,
}

impl BackendTarget {
    #[cfg(windows)]
    pub fn new(
        port: u16,
        generation: Uuid,
        expected_engine_pid: u32,
        liveness: Arc<dyn Fn() -> bool + Send + Sync>,
    ) -> Result<Self, RelayError> {
        if port == 0 || generation.is_nil() || expected_engine_pid == 0 {
            return Err(RelayError::InvalidTarget);
        }
        Ok(Self {
            address: SocketAddrV4::new(std::net::Ipv4Addr::LOCALHOST, port),
            generation,
            expected_engine_pid,
            liveness,
        })
    }

    #[cfg(target_os = "macos")]
    pub fn new_unix(
        socket_path: PathBuf,
        generation: Uuid,
        expected_engine_pid: u32,
        liveness: Arc<dyn Fn() -> bool + Send + Sync>,
    ) -> Result<Self, RelayError> {
        use std::os::unix::ffi::OsStrExt;
        if !socket_path.is_absolute()
            || socket_path.as_os_str().as_bytes().len() >= 104
            || generation.is_nil()
            || expected_engine_pid == 0
            || expected_engine_pid > i32::MAX as u32
        {
            return Err(RelayError::InvalidTarget);
        }
        Ok(Self {
            socket_path,
            generation,
            expected_engine_pid,
            liveness,
        })
    }

    fn is_alive(&self) -> bool {
        catch_unwind(AssertUnwindSafe(|| (self.liveness)())).unwrap_or(false)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct RelayLimits {
    /// Counts accepted TCP connections (not HTTP requests or streams).
    pub max_connections: usize,
    pub buffer_bytes: usize,
    pub connect_timeout: Duration,
    pub idle_timeout: Duration,
    pub lifetime: Duration,
    pub cancellation_grace: Duration,
}

impl Default for RelayLimits {
    fn default() -> Self {
        Self {
            max_connections: 32,
            buffer_bytes: 16 * 1024,
            connect_timeout: Duration::from_secs(2),
            idle_timeout: Duration::from_secs(300),
            lifetime: Duration::from_secs(8 * 60 * 60),
            cancellation_grace: Duration::from_secs(2),
        }
    }
}

impl RelayLimits {
    fn validate(self) -> Result<Self, RelayError> {
        if !(1..=MAX_CONNECTIONS_HARD_LIMIT).contains(&self.max_connections)
            || !(1..=MAX_BUFFER_BYTES).contains(&self.buffer_bytes)
            || self.connect_timeout.is_zero()
            || self.idle_timeout.is_zero()
            || self.lifetime.is_zero()
            || self.cancellation_grace.is_zero()
            || self.connect_timeout > MAX_TIMEOUT
            || self.idle_timeout > MAX_TIMEOUT
            || self.lifetime > MAX_TIMEOUT
            || self.cancellation_grace > MAX_TIMEOUT
        {
            return Err(RelayError::InvalidLimits);
        }
        Ok(self)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RelayError {
    InvalidTarget,
    InvalidLimits,
    NoRoute,
    Draining,
    ListenerUnavailable,
    PeerOwnerUnknown,
    PeerOwnerMissing,
    PeerOwnerTableUnavailable,
    PeerOwnerMismatch,
    PeerOwnerAmbiguous,
    TargetNotAlive,
    ConnectFailed,
    RelayIo,
    NonblockingFailed,
    ClientReadFailed,
    BackendWriteFailed,
    BackendReadFailed,
    ClientWriteFailed,
    IdleTimeout,
    LifetimeTimeout,
    Cancelled,
    ConnectionLimit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DrainReport {
    /// All tracked TCP relay workers have exited.
    pub complete: bool,
    /// The connections ended naturally, without forced socket cancellation.
    pub graceful: bool,
    pub active_connection_count: usize,
    pub cancellations_requested: usize,
    pub listener_released: bool,
}

#[derive(Default)]
struct ControlState {
    route: Option<BackendTarget>,
    draining: bool,
    stopping: bool,
    active: HashMap<u64, ActiveConnection>,
    last_failure: Option<RelayError>,
}

struct ActiveConnection {
    #[cfg(test)]
    generation: Uuid,
    session: Arc<ConnectionControl>,
}

struct Shared {
    control: Mutex<ControlState>,
    changed: Condvar,
    limits: RelayLimits,
    accept_stop: AtomicBool,
    next_id: AtomicU64,
}

struct ConnectionControl {
    cancelled: AtomicBool,
    #[cfg(test)]
    downstream_would_block: AtomicU64,
    #[cfg(test)]
    downstream_bytes_written: AtomicU64,
    started: Instant,
    last_activity: Mutex<Instant>,
    sockets: Mutex<Option<(TcpStream, BackendStream)>>,
}

impl ConnectionControl {
    fn new() -> Self {
        let now = Instant::now();
        Self {
            cancelled: AtomicBool::new(false),
            #[cfg(test)]
            downstream_would_block: AtomicU64::new(0),
            #[cfg(test)]
            downstream_bytes_written: AtomicU64::new(0),
            started: now,
            last_activity: Mutex::new(now),
            sockets: Mutex::new(None),
        }
    }

    fn note_activity(&self) {
        if let Ok(mut last) = self.last_activity.lock() {
            *last = Instant::now();
        }
    }

    fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
        if let Ok(sockets) = self.sockets.lock() {
            if let Some((client, backend)) = sockets.as_ref() {
                let _ = client.shutdown(Shutdown::Both);
                let _ = backend.shutdown(Shutdown::Both);
            }
        }
    }

    fn set_sockets(&self, client: &TcpStream, backend: &BackendStream) -> io::Result<()> {
        let client_control = client.try_clone()?;
        let backend_control = backend.try_clone()?;
        let mut sockets = self
            .sockets
            .lock()
            .map_err(|_| io::Error::other("connection control unavailable"))?;
        *sockets = Some((client_control, backend_control));
        if self.cancelled.load(Ordering::Acquire) {
            if let Some((client, backend)) = sockets.as_ref() {
                let _ = client.shutdown(Shutdown::Both);
                let _ = backend.shutdown(Shutdown::Both);
            }
        }
        Ok(())
    }
}

pub struct IngressRelay {
    stable: Option<StableIngress>,
    address: SocketAddr,
    shared: Arc<Shared>,
    accept_thread: Option<JoinHandle<()>>,
}

impl IngressRelay {
    #[cfg(target_os = "macos")]
    pub fn owns_endpoint(&self) -> bool {
        self.is_accepting() && self.stable.as_ref().is_some_and(StableIngress::is_owned)
    }
    pub fn new(stable: StableIngress, limits: RelayLimits) -> Result<Self, RelayError> {
        let limits = limits.validate()?;
        let address = stable
            .listener()
            .local_addr()
            .map_err(|_| RelayError::ListenerUnavailable)?;
        if !matches!(address, SocketAddr::V4(v4) if v4.ip().is_loopback()) {
            return Err(RelayError::ListenerUnavailable);
        }
        stable
            .listener()
            .set_nonblocking(true)
            .map_err(|_| RelayError::ListenerUnavailable)?;
        let listener = stable
            .listener()
            .try_clone()
            .map_err(|_| RelayError::ListenerUnavailable)?;
        listener
            .set_nonblocking(true)
            .map_err(|_| RelayError::ListenerUnavailable)?;
        let shared = Arc::new(Shared {
            control: Mutex::new(ControlState::default()),
            changed: Condvar::new(),
            limits,
            accept_stop: AtomicBool::new(false),
            next_id: AtomicU64::new(1),
        });
        let thread_shared = Arc::clone(&shared);
        let accept_thread = thread::Builder::new()
            .name("headroom-ingress-accept".to_owned())
            .spawn(move || accept_loop(listener, thread_shared))
            .map_err(|_| RelayError::ListenerUnavailable)?;
        Ok(Self {
            stable: Some(stable),
            address,
            shared,
            accept_thread: Some(accept_thread),
        })
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.address
    }

    pub fn endpoint_id(&self) -> Option<&str> {
        self.stable.as_ref().map(StableIngress::endpoint_id)
    }

    pub fn is_accepting(&self) -> bool {
        self.stable.is_some()
            && self
                .accept_thread
                .as_ref()
                .is_some_and(|thread| !thread.is_finished())
    }

    pub fn route_generation(&self) -> Option<Uuid> {
        self.shared
            .control
            .lock()
            .ok()
            .and_then(|control| control.route.as_ref().map(|route| route.generation))
    }

    /// Future connections use this target. Existing connections retain their
    /// cloned target and are never moved between generations.
    pub fn set_backend(&self, target: BackendTarget) -> Result<(), RelayError> {
        let mut control = self
            .shared
            .control
            .lock()
            .map_err(|_| RelayError::NoRoute)?;
        if control.stopping || (control.draining && !control.active.is_empty()) {
            return Err(RelayError::Draining);
        }
        control.route = Some(target);
        control.draining = false;
        control.last_failure = None;
        Ok(())
    }

    pub fn active_connection_count(&self) -> usize {
        self.shared
            .control
            .lock()
            .map(|control| control.active.len())
            .unwrap_or(0)
    }

    /// Fixed diagnostic only; callers cannot obtain stream or process details.
    #[cfg(test)]
    pub fn last_fixed_failure(&self) -> Option<RelayError> {
        self.shared
            .control
            .lock()
            .ok()
            .and_then(|control| control.last_failure)
    }

    #[cfg(test)]
    fn active_generation_count(&self, generation: Uuid) -> usize {
        self.shared
            .control
            .lock()
            .map(|control| {
                control
                    .active
                    .values()
                    .filter(|active| active.generation == generation)
                    .count()
            })
            .unwrap_or(0)
    }

    /// Removes the route immediately. Accepted connections with no route are
    /// closed before their client payload is read.
    pub fn begin_drain(&self) -> Result<(), RelayError> {
        let mut control = self
            .shared
            .control
            .lock()
            .map_err(|_| RelayError::NoRoute)?;
        control.route = None;
        control.draining = true;
        Ok(())
    }

    /// Waits for existing TCP connections. If the graceful budget expires,
    /// cancels remaining sockets and waits one bounded cancellation grace.
    pub fn finish_drain(&self, timeout: Duration) -> DrainReport {
        let deadline = Instant::now() + timeout.min(MAX_TIMEOUT);
        if self.wait_for_empty(deadline) {
            return self.drain_report(true, true, 0, false);
        }
        let active = self.active_controls();
        for session in &active {
            session.cancel();
        }
        let cancel_deadline = Instant::now() + self.shared.limits.cancellation_grace;
        let complete = self.wait_for_empty(cancel_deadline);
        self.drain_report(complete, false, active.len(), false)
    }

    /// Stops new accepts and releases the stable listener after the bounded
    /// drain attempt. `complete` remains false if any connection outlives it.
    pub fn shutdown(&mut self, timeout: Duration) -> DrainReport {
        let timeout = timeout.min(MAX_TIMEOUT);
        let started = Instant::now();
        let overall_deadline = started + timeout;
        if let Ok(mut control) = self.shared.control.lock() {
            control.route = None;
            control.draining = true;
            control.stopping = true;
        }
        self.shared.accept_stop.store(true, Ordering::Release);
        self.shared.changed.notify_all();

        let accept_reserve = ACCEPT_JOIN_RESERVE.min(timeout / 3);
        let cancel_reserve = self.shared.limits.cancellation_grace.min(timeout / 3);
        let graceful_budget = timeout.saturating_sub(accept_reserve + cancel_reserve);
        let graceful = self.wait_for_empty(started + graceful_budget);
        let mut cancellations_requested = 0;
        let mut complete = graceful;
        if !graceful {
            let active = self.active_controls();
            cancellations_requested = active.len();
            for session in active {
                session.cancel();
            }
            let cancellation_deadline = (Instant::now() + cancel_reserve).min(overall_deadline);
            complete = self.wait_for_empty(cancellation_deadline);
        }

        let accept_joined = self.join_accept_thread_until(overall_deadline);
        if accept_joined {
            self.stable.take();
        }
        let active_connection_count = self.active_connection_count();
        DrainReport {
            complete: complete && active_connection_count == 0,
            graceful,
            active_connection_count,
            cancellations_requested,
            listener_released: accept_joined,
        }
    }

    fn active_controls(&self) -> Vec<Arc<ConnectionControl>> {
        self.shared
            .control
            .lock()
            .map(|control| {
                control
                    .active
                    .values()
                    .map(|active| Arc::clone(&active.session))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn wait_for_empty(&self, deadline: Instant) -> bool {
        let Ok(mut control) = self.shared.control.lock() else {
            return false;
        };
        while !control.active.is_empty() {
            let now = Instant::now();
            if now >= deadline {
                return false;
            }
            let remaining = deadline.saturating_duration_since(now);
            match self.shared.changed.wait_timeout(control, remaining) {
                Ok((next, wait)) => {
                    control = next;
                    if wait.timed_out() && !control.active.is_empty() {
                        return false;
                    }
                }
                Err(_) => return false,
            }
        }
        true
    }

    fn drain_report(
        &self,
        complete: bool,
        graceful: bool,
        cancellations_requested: usize,
        listener_released: bool,
    ) -> DrainReport {
        DrainReport {
            complete,
            graceful,
            active_connection_count: self.active_connection_count(),
            cancellations_requested,
            listener_released,
        }
    }

    fn join_accept_thread_until(&mut self, deadline: Instant) -> bool {
        let Some(thread) = self.accept_thread.as_ref() else {
            return true;
        };
        while !thread.is_finished() {
            if Instant::now() >= deadline {
                return false;
            }
            thread::sleep(ACCEPT_JOIN_RESERVE.min(Duration::from_millis(5)));
        }
        self.accept_thread
            .take()
            .map(|thread| thread.join().is_ok())
            .unwrap_or(true)
    }
}

impl Drop for IngressRelay {
    fn drop(&mut self) {
        if let Ok(mut control) = self.shared.control.lock() {
            control.route = None;
            control.draining = true;
            control.stopping = true;
        }
        for session in self.active_controls() {
            session.cancel();
        }
        self.shared.accept_stop.store(true, Ordering::Release);
        self.shared.changed.notify_all();
        let deadline = Instant::now() + ACCEPT_JOIN_RESERVE;
        let _ = self.join_accept_thread_until(deadline);
        if self.accept_thread.is_none() {
            self.stable.take();
        }
    }
}

fn accept_loop(listener: TcpListener, shared: Arc<Shared>) {
    let mut workers: Vec<JoinHandle<()>> = Vec::new();
    while !shared.accept_stop.load(Ordering::Acquire) {
        reap_workers(&mut workers);
        match listener.accept() {
            Ok((client, _)) => {
                let (id, target, session) = match register_connection(&shared) {
                    Ok(registered) => registered,
                    Err(error) => {
                        if let Ok(mut control) = shared.control.lock() {
                            control.last_failure = Some(error);
                        }
                        drop(client);
                        continue;
                    }
                };
                let worker_shared = Arc::clone(&shared);
                let spawned = thread::Builder::new()
                    .name("headroom-ingress-connection".to_owned())
                    .spawn(move || {
                        let outcome = run_connection(
                            client,
                            target,
                            Arc::clone(&session),
                            worker_shared.limits,
                        );
                        if let Ok(mut control) = worker_shared.control.lock() {
                            control.active.remove(&id);
                            if let Err(error) = outcome {
                                control.last_failure = Some(error);
                            }
                            worker_shared.changed.notify_all();
                        }
                    });
                match spawned {
                    Ok(worker) => workers.push(worker),
                    Err(_) => {
                        if let Ok(mut control) = shared.control.lock() {
                            control.active.remove(&id);
                            control.last_failure = Some(RelayError::ListenerUnavailable);
                            shared.changed.notify_all();
                        }
                    }
                }
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(ACCEPT_POLL_INTERVAL);
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        }
    }
    // Finished workers are joined. Active workers are bounded by per-connection
    // deadlines; on an incomplete shutdown they are detached after cancellation.
    reap_workers(&mut workers);
}

fn reap_workers(workers: &mut Vec<JoinHandle<()>>) {
    let mut index = 0;
    while index < workers.len() {
        if workers[index].is_finished() {
            let worker = workers.swap_remove(index);
            let _ = worker.join();
        } else {
            index += 1;
        }
    }
}

fn register_connection(
    shared: &Shared,
) -> Result<(u64, BackendTarget, Arc<ConnectionControl>), RelayError> {
    let mut control = shared
        .control
        .lock()
        .map_err(|_| RelayError::ListenerUnavailable)?;
    if control.stopping || control.draining || shared.accept_stop.load(Ordering::Acquire) {
        return Err(RelayError::Draining);
    }
    let target = control.route.clone().ok_or(RelayError::NoRoute)?;
    if control.active.len() >= shared.limits.max_connections {
        return Err(RelayError::ConnectionLimit);
    }
    let id = shared.next_id.fetch_add(1, Ordering::Relaxed);
    let session = Arc::new(ConnectionControl::new());
    control.active.insert(
        id,
        ActiveConnection {
            #[cfg(test)]
            generation: target.generation,
            session: Arc::clone(&session),
        },
    );
    Ok((id, target, session))
}

fn run_connection(
    client: TcpStream,
    target: BackendTarget,
    session: Arc<ConnectionControl>,
    limits: RelayLimits,
) -> Result<(), RelayError> {
    if !target.is_alive() || session.cancelled.load(Ordering::Acquire) {
        return Err(if session.cancelled.load(Ordering::Acquire) {
            RelayError::Cancelled
        } else {
            RelayError::TargetNotAlive
        });
    }
    #[cfg(windows)]
    let backend_address = SocketAddr::V4(target.address);
    #[cfg(windows)]
    let Ok(backend) = TcpStream::connect_timeout(&backend_address, limits.connect_timeout) else {
        return Err(RelayError::ConnectFailed);
    };
    #[cfg(target_os = "macos")]
    let backend = crate::runtime::unix_backend::UnixTarget {
        socket_path: &target.socket_path,
        expected_pid: target.expected_engine_pid,
    }
    .connect_with_timeout(limits.connect_timeout, || target.is_alive())
    .map_err(|error| match error {
        crate::runtime::unix_backend::UnixBackendError::Transport => RelayError::ConnectFailed,
        _ => RelayError::PeerOwnerMismatch,
    })?;
    if !target.is_alive() || session.cancelled.load(Ordering::Acquire) {
        return Err(if session.cancelled.load(Ordering::Acquire) {
            RelayError::Cancelled
        } else {
            RelayError::TargetNotAlive
        });
    }
    #[cfg(windows)]
    verify_backend_peer_before_payload(&backend, target.expected_engine_pid).map_err(|error| {
        match error {
            IngressError::PeerOwnerMismatch => RelayError::PeerOwnerMismatch,
            IngressError::PeerOwnerAmbiguous => RelayError::PeerOwnerAmbiguous,
            IngressError::PeerOwnerMissing => RelayError::PeerOwnerMissing,
            IngressError::PeerOwnerTableUnavailable => RelayError::PeerOwnerTableUnavailable,
            _ => RelayError::PeerOwnerUnknown,
        }
    })?;
    if !target.is_alive() || session.cancelled.load(Ordering::Acquire) {
        return Err(if session.cancelled.load(Ordering::Acquire) {
            RelayError::Cancelled
        } else {
            RelayError::TargetNotAlive
        });
    }
    session
        .set_sockets(&client, &backend)
        .map_err(|_| RelayError::RelayIo)?;
    if session.cancelled.load(Ordering::Acquire) {
        session.cancel();
        return Err(RelayError::Cancelled);
    }
    let _ = client.set_nodelay(true);
    #[cfg(windows)]
    let _ = backend.set_nodelay(true);
    relay_bidirectional(client, backend, session, limits)
}

/// Windows may publish the server-side TCP owner row just after connect. Only
/// a missing row receives a tiny bounded visibility wait; a known PID
/// mismatch or ambiguous tuple fails immediately. No client byte is read while
/// this check runs.
#[cfg(windows)]
fn verify_backend_peer_before_payload(
    backend: &TcpStream,
    expected_engine_pid: u32,
) -> Result<(), IngressError> {
    let deadline = Instant::now() + PEER_ROW_VISIBILITY_BUDGET;
    loop {
        match verify_backend_peer(backend, expected_engine_pid) {
            Err(IngressError::PeerOwnerMissing) if Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(2));
            }
            result => return result,
        }
    }
}

fn relay_bidirectional(
    mut client: TcpStream,
    mut backend: BackendStream,
    session: Arc<ConnectionControl>,
    limits: RelayLimits,
) -> Result<(), RelayError> {
    if client.set_nonblocking(true).is_err() || backend.set_nonblocking(true).is_err() {
        session.cancel();
        return Err(RelayError::NonblockingFailed);
    }
    let mut upstream = PipeBuffer::new(limits.buffer_bytes);
    let mut downstream = PipeBuffer::new(limits.buffer_bytes);
    loop {
        if let Err(error) = check_deadlines(&session, limits) {
            session.cancel();
            return Err(error);
        }
        let mut progressed = false;

        if !upstream.source_eof && upstream.can_read() {
            match client.read(upstream.read_slot()) {
                Ok(0) => {
                    upstream.source_eof = true;
                    progressed = true;
                }
                Ok(read) => {
                    upstream.record_read(read);
                    session.note_activity();
                    progressed = true;
                }
                Err(error) if is_retryable_nonblocking(&error) => {}
                Err(_) => {
                    session.cancel();
                    return Err(RelayError::ClientReadFailed);
                }
            }
        }
        if !upstream.is_empty() {
            match backend.write(upstream.pending()) {
                Ok(0) => {
                    session.cancel();
                    return Err(RelayError::BackendWriteFailed);
                }
                Ok(written) => {
                    upstream.consume(written);
                    session.note_activity();
                    progressed = true;
                }
                Err(error) if is_retryable_nonblocking(&error) => {}
                Err(_) => {
                    session.cancel();
                    return Err(RelayError::BackendWriteFailed);
                }
            }
        }
        if upstream.source_eof && upstream.is_empty() && !upstream.write_shutdown {
            let _ = backend.shutdown(Shutdown::Write);
            upstream.write_shutdown = true;
            progressed = true;
        }

        if !downstream.source_eof && downstream.can_read() {
            match backend.read(downstream.read_slot()) {
                Ok(0) => {
                    downstream.source_eof = true;
                    progressed = true;
                }
                Ok(read) => {
                    downstream.record_read(read);
                    session.note_activity();
                    progressed = true;
                }
                Err(error) if is_retryable_nonblocking(&error) => {}
                Err(_) => {
                    session.cancel();
                    return Err(RelayError::BackendReadFailed);
                }
            }
        }
        if !downstream.is_empty() {
            match client.write(downstream.pending()) {
                Ok(0) => {
                    session.cancel();
                    return Err(RelayError::ClientWriteFailed);
                }
                Ok(written) => {
                    #[cfg(test)]
                    session
                        .downstream_bytes_written
                        .fetch_add(written as u64, Ordering::Relaxed);
                    downstream.consume(written);
                    session.note_activity();
                    progressed = true;
                }
                Err(error) if is_retryable_nonblocking(&error) => {
                    #[cfg(test)]
                    if error.kind() == io::ErrorKind::WouldBlock {
                        session
                            .downstream_would_block
                            .fetch_add(1, Ordering::Relaxed);
                    }
                }
                Err(_) => {
                    session.cancel();
                    return Err(RelayError::ClientWriteFailed);
                }
            }
        }
        if downstream.source_eof && downstream.is_empty() && !downstream.write_shutdown {
            let _ = client.shutdown(Shutdown::Write);
            downstream.write_shutdown = true;
            progressed = true;
        }

        if upstream.source_eof
            && upstream.is_empty()
            && downstream.source_eof
            && downstream.is_empty()
        {
            return Ok(());
        }
        if !progressed {
            thread::sleep(Duration::from_millis(2));
        }
    }
}

struct PipeBuffer {
    bytes: Vec<u8>,
    start: usize,
    end: usize,
    source_eof: bool,
    write_shutdown: bool,
}

impl PipeBuffer {
    fn new(capacity: usize) -> Self {
        Self {
            bytes: vec![0; capacity],
            start: 0,
            end: 0,
            source_eof: false,
            write_shutdown: false,
        }
    }

    fn can_read(&mut self) -> bool {
        if self.start == self.end {
            self.start = 0;
            self.end = 0;
        } else if self.end == self.bytes.len() && self.start > 0 {
            let remaining = self.end - self.start;
            self.bytes.copy_within(self.start..self.end, 0);
            self.start = 0;
            self.end = remaining;
        }
        self.end < self.bytes.len()
    }

    fn read_slot(&mut self) -> &mut [u8] {
        &mut self.bytes[self.end..]
    }

    fn record_read(&mut self, count: usize) {
        self.end += count;
    }

    fn is_empty(&self) -> bool {
        self.start == self.end
    }

    fn pending(&self) -> &[u8] {
        &self.bytes[self.start..self.end]
    }

    fn consume(&mut self, count: usize) {
        self.start += count;
        if self.start == self.end {
            self.start = 0;
            self.end = 0;
        }
    }
}

fn is_retryable_nonblocking(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
    )
}

fn check_deadlines(session: &ConnectionControl, limits: RelayLimits) -> Result<(), RelayError> {
    if session.cancelled.load(Ordering::Acquire) {
        return Err(RelayError::Cancelled);
    }
    let now = Instant::now();
    if now.saturating_duration_since(session.started) >= limits.lifetime {
        return Err(RelayError::LifetimeTimeout);
    }
    let last_activity = session
        .last_activity
        .lock()
        .map(|value| *value)
        .unwrap_or(session.started);
    if now.saturating_duration_since(last_activity) >= limits.idle_timeout {
        return Err(RelayError::IdleTimeout);
    }
    Ok(())
}

#[cfg(all(test, windows))]
#[path = "ingress_relay_tests.rs"]
mod tests;

#[cfg(all(test, target_os = "macos"))]
#[path = "ingress_relay_macos_tests.rs"]
mod tests;
