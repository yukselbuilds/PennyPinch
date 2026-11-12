use super::*;
use core_foundation::{
    base::{CFType, CFTypeRef, TCFType},
    data::CFData,
    dictionary::CFDictionary,
    propertylist::{create_with_data, kCFPropertyListImmutable},
    string::{CFString, CFStringRef},
    url::{CFURLRef, CFURL},
};

const BUNDLE_IDENTIFIER: &str = "com.openai.codex";
const BUNDLE_REQUIREMENT: &str = r#"identifier "com.openai.codex" and anchor apple generic and certificate leaf[subject.OU] = "2DC432GLL2""#;
const MAX_PLIST_BYTES: u64 = 64 * 1024;
const MAX_LAUNCHER_BYTES: u64 = 64 * 1024 * 1024;
const SEC_CS_DO_NOT_VALIDATE_RESOURCES: u32 = 1 << 2;
const SEC_CS_STRICT_VALIDATE: u32 = 1 << 4;
const SEC_CS_NO_NETWORK_ACCESS: u32 = 1 << 29;

#[link(name = "Security", kind = "framework")]
unsafe extern "C" {
    fn SecStaticCodeCreateWithPath(path: CFURLRef, flags: u32, code: *mut CFTypeRef) -> i32;
    fn SecRequirementCreateWithString(
        text: CFStringRef,
        flags: u32,
        requirement: *mut CFTypeRef,
    ) -> i32;
    fn SecStaticCodeCheckValidity(code: CFTypeRef, flags: u32, requirement: CFTypeRef) -> i32;
}

pub(in crate::integrations::discovery) fn add_desktop_candidates(
    inputs: &DiscoveryInputs,
    candidates: &mut HashMap<String, Candidate>,
    warnings: &mut BTreeSet<&'static str>,
    candidate_count: &mut usize,
) -> Observations {
    let user_applications = inputs
        .macos_home
        .as_ref()
        .map(|home| home.join("Applications"));
    let mut roots = HashSet::new();
    let mut observations = HashMap::new();
    for applications in inputs.macos_applications.iter().chain(&user_applications) {
        for name in ["ChatGPT.app", "Codex.app"] {
            if *candidate_count >= MAX_CANDIDATES {
                warnings.insert("candidate_limit_reached");
                return observations;
            }
            *candidate_count += 1;
            let Some(bundle) =
                checked_existing_path(&applications.join(name), PathKind::Directory, warnings)
            else {
                continue;
            };
            if !roots.insert(bundle.clone()) {
                continue;
            }
            let Some((candidate, observed)) = inspect_bundle(bundle) else {
                warnings.insert("desktop_metadata_untrusted");
                continue;
            };
            observations.insert(
                (
                    candidate.kind,
                    candidate.surface,
                    candidate.executable_path.clone(),
                ),
                observed,
            );
            insert_candidate(candidates, candidate);
        }
    }
    observations
}

fn inspect_bundle(bundle: PathBuf) -> Option<(Candidate, InstallationObservation)> {
    let plist_path =
        check_existing_path(&bundle.join("Contents/Info.plist"), PathKind::File).ok()?;
    let mut plist_file = open_command(&plist_path)?;
    let plist_stamp = trusted_file_metadata(&plist_file)?;
    if plist_stamp.len() > MAX_PLIST_BYTES {
        return None;
    }
    let mut bytes = Vec::new();
    (&mut plist_file)
        .take(MAX_PLIST_BYTES + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() as u64 != plist_stamp.len() {
        return None;
    }

    let (parsed, _) =
        create_with_data(CFData::from_buffer(&bytes), kCFPropertyListImmutable).ok()?;
    // Core Foundation returns a retained property list with string dictionary keys.
    let info = unsafe { CFType::wrap_under_create_rule(parsed) }.downcast_into::<CFDictionary>()?;
    let string = |key| {
        let key = CFString::new(key);
        let value = info.find(key.as_CFTypeRef())?;
        // An immutable property list dictionary keeps each returned value alive.
        unsafe { CFType::wrap_under_get_rule(*value) }
            .downcast::<CFString>()
            .map(|value| value.to_string())
    };
    if string("CFBundleIdentifier").as_deref() != Some(BUNDLE_IDENTIFIER)
        || string("CFBundlePackageType").as_deref() != Some("APPL")
    {
        return None;
    }
    let executable = string("CFBundleExecutable")?;
    if !matches!(executable.as_str(), "ChatGPT" | "Codex") {
        return None;
    }
    let executable_path = check_existing_path(
        &bundle.join("Contents/MacOS").join(executable),
        PathKind::File,
    )
    .ok()?;
    let launcher = open_command(&executable_path)?;
    let stamp = trusted_file_metadata(&launcher)?;
    if stamp.len() > MAX_LAUNCHER_BYTES || stamp.mode() & 0o111 == 0 {
        return None;
    }
    verify_bundle(&bundle)?;
    file_is_current(&plist_file, &plist_stamp, &plist_path)?;
    file_is_current(&launcher, &stamp, &executable_path)?;
    Some((
        Candidate {
            kind: "codex",
            surface: "desktop",
            version: string("CFBundleShortVersionString").filter(|version| {
                super::super::super::codex_binary::desktop_version_supported(version)
            }),
            source: "macos_app_bundle",
            location_label: "macOS application",
            identity_path: bundle,
            executable_path,
        },
        observation_from_metadata(&stamp)?,
    ))
}

pub(in crate::integrations::discovery) fn observe_desktop(
    path: &Path,
    version: Option<&str>,
) -> Option<InstallationObservation> {
    let bundle = path.ancestors().nth(3)?.to_owned();
    let (candidate, observed) = inspect_bundle(bundle)?;
    (candidate.executable_path == path && candidate.version.as_deref() == version)
        .then_some(observed)
}

fn verify_bundle(bundle: &Path) -> Option<()> {
    let url = CFURL::from_path(bundle, true)?;
    let text = CFString::new(BUNDLE_REQUIREMENT);
    let mut code = std::ptr::null();
    let mut requirement = std::ptr::null();
    // The two Create calls return retained CF objects. Discovery validates the launcher
    // and Info.plist offline, while the application resources are outside this check.
    unsafe {
        if SecStaticCodeCreateWithPath(url.as_concrete_TypeRef(), 0, &mut code) != 0
            || code.is_null()
        {
            return None;
        }
        let code = CFType::wrap_under_create_rule(code);
        if SecRequirementCreateWithString(text.as_concrete_TypeRef(), 0, &mut requirement) != 0
            || requirement.is_null()
        {
            return None;
        }
        let requirement = CFType::wrap_under_create_rule(requirement);
        let flags =
            SEC_CS_DO_NOT_VALIDATE_RESOURCES | SEC_CS_STRICT_VALIDATE | SEC_CS_NO_NETWORK_ACCESS;
        (SecStaticCodeCheckValidity(code.as_CFTypeRef(), flags, requirement.as_CFTypeRef()) == 0)
            .then_some(())
    }
}

#[cfg(test)]
#[path = "discovery_macos_desktop_tests.rs"]
mod tests;
