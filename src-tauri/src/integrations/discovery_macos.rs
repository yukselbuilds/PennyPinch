use super::*;
use std::fs::Metadata;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};

const MAX_LINKS: usize = 8;
const MAX_HASH_BYTES: u64 = 1024 * 1024 * 1024;

// Official Apple Silicon release binaries, measured without executing them.
const PINNED_CLI: &[(&str, &str, u64, &str)] = &[
    (
        "codex",
        "0.163.0-alpha.11.1",
        249_449_504,
        "c43dfdedd0f4c6f9e2c8e03070233db63ad041cc299f53b57a325f06494d1954",
    ),
    (
        "claude_code",
        "2.1.158",
        215_233_824,
        "536a0517fa64d48ddcbc8eb511a3d08027d47e06d148872332a8041d72c22768",
    ),
    (
        "opencode",
        "1.19.3",
        144_634_848,
        "6d3e6458fce4b649edf974c19959d5dbfe1e7f255a503dcfc01215b24ad3ea5b",
    ),
];

pub(super) fn add_cli_candidates(
    inputs: &DiscoveryInputs,
    candidates: &mut HashMap<String, Candidate>,
    warnings: &mut BTreeSet<&'static str>,
    candidate_count: &mut usize,
) {
    let home_bins = inputs
        .macos_home
        .iter()
        .filter(|home| home.is_absolute())
        .flat_map(|home| [home.join(".local/bin"), home.join(".opencode/bin")]);
    let roots = home_bins
        .map(|root| (root, "user_local_native", "User-local install"))
        .chain(
            inputs
                .path_entries
                .iter()
                .take(MAX_PATH_ENTRIES)
                .cloned()
                .map(|root| (root, "path", "PATH")),
        );
    let mut seen_dirs = HashSet::new();
    let mut seen_files = HashSet::new();
    let mut hash_budget = MAX_HASH_BYTES;
    for (root, source, location_label) in roots {
        if is_network_or_device_path(&root) {
            warnings.insert("network_location_skipped");
            continue;
        }
        let Some(root) = checked_existing_path(&root, PathKind::Directory, warnings) else {
            continue;
        };
        if !seen_dirs.insert(root.clone()) {
            continue;
        }
        for product in PRODUCTS {
            if *candidate_count == MAX_CANDIDATES {
                warnings.insert("candidate_limit_reached");
                return;
            }
            *candidate_count += 1;
            let Some(path) = resolve_command(root.join(product.command)) else {
                continue;
            };
            let Some(version) =
                inspect_command(product.kind, &path, &mut hash_budget, &mut seen_files)
            else {
                continue;
            };
            insert_candidate(
                candidates,
                Candidate {
                    kind: product.kind,
                    surface: "cli",
                    version,
                    source,
                    location_label,
                    identity_path: path.clone(),
                    executable_path: path,
                },
            );
        }
    }
}

fn resolve_command(mut path: PathBuf) -> Option<PathBuf> {
    for _ in 0..=MAX_LINKS {
        let parent = check_existing_path(path.parent()?, PathKind::Directory).ok()?;
        path = parent.join(path.file_name()?);
        let metadata = fs::symlink_metadata(&path).ok()?;
        if !metadata.file_type().is_symlink() {
            return check_existing_path(&path, PathKind::File).ok();
        }
        let target = fs::read_link(&path).ok()?;
        let mut resolved = parent;
        let mut entered_target = false;
        for part in target.components() {
            match part {
                Component::RootDir => resolved = PathBuf::from("/"),
                Component::CurDir => {}
                Component::Normal(name) => {
                    resolved.push(name);
                    entered_target = true;
                }
                // Only leading `..` is relative to a checked directory.
                Component::ParentDir if !entered_target && resolved.pop() => {}
                _ => return None,
            }
        }
        path = resolved;
    }
    None
}

fn open_command(path: &Path) -> Option<File> {
    OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW_ANY | libc::O_NONBLOCK | libc::O_CLOEXEC)
        .open(path)
        .ok()
}

fn inspect_command(
    kind: &'static str,
    path: &Path,
    hash_budget: &mut u64,
    seen_files: &mut HashSet<(&'static str, u64, u64)>,
) -> Option<Option<String>> {
    let mut file = open_command(path)?;
    let before = file.metadata().ok()?;
    if !before.is_file()
        || before.nlink() != 1
        || before.len() == 0
        || before.mode() & 0o111 == 0
        || before.mode() & 0o7022 != 0
        || (before.uid() != 0 && before.uid() != unsafe { libc::geteuid() })
    {
        return None;
    }
    let identity = (kind, before.dev(), before.ino());
    if seen_files.contains(&identity) {
        return None;
    }
    let version = if let Some(&(_, version, size, expected_hash)) =
        PINNED_CLI.iter().find(|&&(client, _, size, _)| {
            client == kind && before.len() == size && *hash_budget >= size
        }) {
        *hash_budget -= size;
        let mut digest = Sha256::new();
        let mut bounded = (&mut file).take(size + 1);
        let mut buffer = [0; 64 * 1024];
        loop {
            let count = bounded.read(&mut buffer).ok()?;
            if count == 0 {
                break;
            }
            digest.update(&buffer[..count]);
        }
        if bounded.limit() != 1 {
            return None;
        }
        (format!("{:x}", digest.finalize()) == expected_hash).then(|| version.to_owned())
    } else {
        None
    };
    let held = file.metadata().ok()?;
    let named = open_command(path)?.metadata().ok()?;
    if file_stamp(&held) != file_stamp(&before) || file_stamp(&named) != file_stamp(&before) {
        return None;
    }
    seen_files.insert(identity);
    Some(version)
}

fn file_stamp(info: &Metadata) -> (u64, u64, u32, u32, u64, u64, i64, i64, i64, i64) {
    (
        info.dev(),
        info.ino(),
        info.uid(),
        info.mode(),
        info.nlink(),
        info.len(),
        info.mtime(),
        info.mtime_nsec(),
        info.ctime(),
        info.ctime_nsec(),
    )
}

#[cfg(test)]
#[path = "discovery_macos_tests.rs"]
mod tests;
