use super::{Failure, UpdateChannel, UpdateError, UpdatePolicy, VerifiedUpdate};
use crate::desktop::update::{verify_manifest, MANIFEST_LIMIT, SIGNATURE_LIMIT};
use crate::integrations::config_io::{pin_existing_directory_chain, PinnedDirectoryChain};
use reqwest::{
    blocking::{Client, Response},
    redirect, StatusCode, Url,
};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom};
use std::os::windows::fs::OpenOptionsExt;
use std::path::Path;
use std::time::Duration;
use uuid::Uuid;
use windows_sys::Win32::Foundation::{GENERIC_READ, GENERIC_WRITE};
use windows_sys::Win32::Storage::FileSystem::{
    DELETE, FILE_ATTRIBUTE_TEMPORARY, FILE_FLAG_DELETE_ON_CLOSE, FILE_FLAG_OPEN_REPARSE_POINT,
    FILE_FLAG_SEQUENTIAL_SCAN,
};

pub(super) struct ReleaseSource {
    base: Url,
    public_key: &'static str,
    local_fixture: bool,
}

impl ReleaseSource {
    pub(super) fn configured() -> Option<Self> {
        if !cfg!(target_arch = "x86_64") {
            return None;
        }
        #[cfg(all(debug_assertions, feature = "update-fixture"))]
        if let Ok(base) = std::env::var("HEADROOM_DESKTOP_UPDATE_FIXTURE_URL") {
            return Self::fixture(&base);
        }
        Self::new(
            option_env!("HEADROOM_DESKTOP_UPDATE_BASE_URL")?,
            option_env!("HEADROOM_DESKTOP_UPDATE_PUBLIC_KEY")?,
            false,
        )
    }

    pub(super) fn new(base: &str, public_key: &'static str, local_fixture: bool) -> Option<Self> {
        let base = Url::parse(base).ok()?;
        let valid_origin = if local_fixture {
            base.scheme() == "http" && base.host_str() == Some("127.0.0.1") && base.port().is_some()
        } else {
            base.scheme() == "https"
                && base.domain().is_some_and(|domain| {
                    let host = domain.trim_end_matches('.');
                    host != "localhost" && !host.ends_with(".localhost")
                })
        };
        if !valid_origin
            || !base.username().is_empty()
            || base.password().is_some()
            || base.query().is_some()
            || base.fragment().is_some()
            || !base.path().ends_with('/')
            || public_key.trim().is_empty()
        {
            return None;
        }
        Some(Self {
            base,
            public_key,
            local_fixture,
        })
    }

    #[cfg(any(test, all(debug_assertions, feature = "update-fixture")))]
    pub(super) fn fixture(base: &str) -> Option<Self> {
        let key = include_str!("../../../tests/fixtures/update/fixture-release.pub");
        Self::new(base, key, true)
    }

    pub(super) fn public_key(&self) -> &str {
        self.public_key
    }

    fn client(&self) -> Result<Client, Failure> {
        Client::builder()
            .https_only(!self.local_fixture)
            .redirect(redirect::Policy::none())
            .no_proxy()
            .min_tls_version(reqwest::tls::Version::TLS_1_2)
            .connect_timeout(Duration::from_secs(5))
            .build()
            .map_err(|_| Failure::SourceFailed)
    }

    fn request(
        &self,
        client: &Client,
        channel: UpdateChannel,
        name: &str,
        timeout: Duration,
    ) -> Result<Response, Failure> {
        let mut url = self.base.clone();
        url.path_segments_mut()
            .map_err(|_| Failure::SourceFailed)?
            .pop_if_empty()
            .push(match channel {
                UpdateChannel::Stable => "stable",
                UpdateChannel::Preview => "preview",
            })
            .push(name);
        let response = client
            .get(url)
            .timeout(timeout)
            .send()
            .map_err(|_| Failure::SourceFailed)?;
        if response.status() != StatusCode::OK {
            return Err(Failure::SourceFailed);
        }
        Ok(response)
    }

    fn metadata(
        &self,
        client: &Client,
        channel: UpdateChannel,
        name: &str,
        limit: usize,
    ) -> Result<Vec<u8>, Failure> {
        let response = self.request(client, channel, name, Duration::from_secs(12))?;
        let mut data = Vec::new();
        response
            .take(limit as u64 + 1)
            .read_to_end(&mut data)
            .map_err(|_| Failure::SourceFailed)?;
        if data.len() > limit {
            return Err(Failure::InvalidRelease);
        }
        Ok(data)
    }

    pub(super) fn check(
        &self,
        policy: &UpdatePolicy<'_>,
    ) -> Result<Option<VerifiedUpdate>, Failure> {
        let client = self.client()?;
        let bytes = self.metadata(&client, policy.channel, "manifest.json", MANIFEST_LIMIT)?;
        let signature = self.metadata(
            &client,
            policy.channel,
            "manifest.json.sig",
            SIGNATURE_LIMIT,
        )?;
        let signature = std::str::from_utf8(&signature).map_err(|_| Failure::InvalidRelease)?;
        match verify_manifest(&bytes, signature, policy) {
            Ok(update) => Ok(Some(update)),
            Err(UpdateError::VersionNotNewer) => Ok(None),
            Err(_) => Err(Failure::InvalidRelease),
        }
    }

    pub(super) fn download(
        &self,
        app_data: &Path,
        update: &VerifiedUpdate,
    ) -> Result<File, Failure> {
        let client = self.client()?;
        let response = self.request(
            &client,
            update.channel(),
            &update.installer_name(),
            Duration::from_secs(300),
        )?;
        let expected_size = update.installer_size();
        if response
            .content_length()
            .is_some_and(|size| size != expected_size)
        {
            return Err(Failure::IntegrityFailed);
        }
        let (mut file, pins) = create_staging_file(app_data)?;
        let size = io::copy(&mut response.take(expected_size + 1), &mut file)
            .map_err(|_| Failure::DownloadFailed)?;
        if size != expected_size {
            return Err(Failure::IntegrityFailed);
        }
        file.sync_all().map_err(|_| Failure::StorageFailed)?;
        file.seek(SeekFrom::Start(0))
            .map_err(|_| Failure::StorageFailed)?;
        update
            .verify_installer(&mut file)
            .map_err(|_| Failure::IntegrityFailed)?;
        pins.validate().map_err(|_| Failure::StorageFailed)?;
        file.seek(SeekFrom::Start(0))
            .map_err(|_| Failure::StorageFailed)?;
        Ok(file)
    }
}

fn create_staging_file(app_data: &Path) -> Result<(File, PinnedDirectoryChain), Failure> {
    let parent = app_data.parent().ok_or(Failure::StorageFailed)?;
    let parent_pins = pin_existing_directory_chain(parent).map_err(|_| Failure::StorageFailed)?;
    match fs::create_dir(app_data) {
        Ok(()) => (),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => (),
        Err(_) => return Err(Failure::StorageFailed),
    }
    parent_pins.validate().map_err(|_| Failure::StorageFailed)?;
    let pins = pin_existing_directory_chain(app_data).map_err(|_| Failure::StorageFailed)?;
    let name = format!(".headroom-update-{}.part", Uuid::new_v4());
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .access_mode(GENERIC_READ | GENERIC_WRITE | DELETE)
        .share_mode(0)
        .custom_flags(
            FILE_ATTRIBUTE_TEMPORARY
                | FILE_FLAG_OPEN_REPARSE_POINT
                | FILE_FLAG_DELETE_ON_CLOSE
                | FILE_FLAG_SEQUENTIAL_SCAN,
        )
        .open(app_data.join(name))
        .map_err(|_| Failure::StorageFailed)?;
    Ok((file, pins))
}
