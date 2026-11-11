//! Read-only OS socket metadata; no connection, packet or process is adopted.

use super::ingress::IngressError;
use std::io::{self, Read};
use std::net::Ipv4Addr;
use std::os::fd::AsRawFd;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug, Eq, PartialEq)]
pub(super) struct PortListeners {
    pub(super) loopback: bool,
    pub(super) wildcard: bool,
}

// Apple emits tcp46 for dual-stack sockets and numeric IPv4 when INP_IPV4 is set.
// Format source: apple-oss-distributions/network_cmds/netstat.tproj/inet.c.
pub(super) fn parse_port_table(table: &str, port: u16) -> Result<PortListeners, IngressError> {
    let mut found = PortListeners {
        loopback: false,
        wildcard: false,
    };
    for line in table.lines().map(str::trim).filter(|line| !line.is_empty()) {
        if line == "Active Internet connections (including servers)"
            || line.split_whitespace().collect::<Vec<_>>()
                == [
                    "Proto", "Recv-Q", "Send-Q", "Local", "Address", "Foreign", "Address",
                    "(state)",
                ]
        {
            continue;
        }
        let row: Vec<_> = line.split_whitespace().collect();
        if row.len() != 6
            || !matches!(row[0], "tcp4" | "tcp46" | "tcp6")
            || row[1].parse::<u64>().is_err()
            || row[2].parse::<u64>().is_err()
        {
            return Err(IngressError::StateUnavailable);
        }
        if row[5] != "LISTEN" || row[0] == "tcp6" {
            continue;
        }
        let (address, endpoint_port) = row[3]
            .rsplit_once('.')
            .ok_or(IngressError::StateUnavailable)?;
        let endpoint_port = endpoint_port
            .parse::<u16>()
            .map_err(|_| IngressError::StateUnavailable)?;
        let address = if address == "*" {
            Ipv4Addr::UNSPECIFIED
        } else {
            address
                .parse::<Ipv4Addr>()
                .map_err(|_| IngressError::StateUnavailable)?
        };
        if endpoint_port == port {
            found.loopback |= address == Ipv4Addr::LOCALHOST;
            found.wildcard |= address.is_unspecified();
        }
    }
    Ok(found)
}

pub(super) fn observe_port(port: u16) -> Result<PortListeners, IngressError> {
    let mut child = Command::new("/usr/sbin/netstat")
        .args(["-an", "-p", "tcp"])
        .env_clear()
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| IngressError::StateUnavailable)?;
    let result = (|| {
        let mut stdout = child.stdout.take().ok_or(IngressError::StateUnavailable)?;
        let fd = stdout.as_raw_fd();
        // SAFETY: stdout owns a live descriptor throughout both fcntl calls.
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
            return Err(IngressError::StateUnavailable);
        }
        let deadline = Instant::now() + Duration::from_secs(1);
        let mut bytes = Vec::new();
        let mut buffer = [0u8; 8192];
        loop {
            if Instant::now() >= deadline {
                return Err(IngressError::StateUnavailable);
            }
            match stdout.read(&mut buffer) {
                Ok(0) => break,
                Ok(count) if bytes.len() + count <= 4 * 1024 * 1024 => {
                    bytes.extend_from_slice(&buffer[..count])
                }
                Ok(_) => return Err(IngressError::StateUnavailable),
                Err(error)
                    if error.kind() == io::ErrorKind::WouldBlock
                        || error.kind() == io::ErrorKind::Interrupted =>
                {
                    thread::sleep(Duration::from_millis(2));
                }
                Err(_) => return Err(IngressError::StateUnavailable),
            }
        }
        loop {
            match child.try_wait() {
                Ok(Some(status)) if status.success() => break,
                Ok(Some(_)) | Err(_) => return Err(IngressError::StateUnavailable),
                Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(2)),
                _ => return Err(IngressError::StateUnavailable),
            }
        }
        let table = std::str::from_utf8(&bytes).map_err(|_| IngressError::StateUnavailable)?;
        parse_port_table(table, port)
    })();
    let reaped = match child.try_wait() {
        Ok(Some(_)) => true,
        Ok(None) => {
            let _ = child.kill();
            let deadline = Instant::now() + Duration::from_millis(250);
            loop {
                match child.try_wait() {
                    Ok(Some(_)) => break true,
                    Ok(None) if Instant::now() < deadline => {
                        thread::sleep(Duration::from_millis(2))
                    }
                    _ => break false,
                }
            }
        }
        Err(_) => false,
    };
    if reaped {
        result
    } else {
        Err(IngressError::StateUnavailable)
    }
}
