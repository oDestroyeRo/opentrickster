use crate::{NetError, Result, wire::Reader};
use flate2::read::GzDecoder;
use futures_util::StreamExt;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::Path,
    time::Duration,
};
use url::Url;
use zeroize::Zeroizing;

pub const BUILD: &str = "8502ebbf2fc9033d2fba50d02fd92002";
pub(crate) const ORIGIN: &str = "https://tr-play.playall.in.th";
const MAX_METADATA: usize = 9_343_896;
struct Asset {
    name: &'static str,
    size: usize,
    hash: &'static str,
}
const ASSETS: [Asset; 4] = [
    Asset {
        name: "41e1cc21f8b1ab11275e55eb6949af3c.loader.js",
        size: 27914,
        hash: "5a4ba6acdfc6ad420a285cc1faa54dc694892655bedd752e0f3f7dde3bae6a41",
    },
    Asset {
        name: "7f982dae9f02da04f526a91e415474b4.framework.js.gz",
        size: 391375,
        hash: "d159059282768082bbfc54432ab93ad68f6eaa57f84883c9e09780810597d917",
    },
    Asset {
        name: "d9a4c4fd2737e740b82e1321839e352f.wasm.gz",
        size: 43136343,
        hash: "381a183efd05b40853fbfe4e052d64edff2b8a6b254773fe37241e7d53789404",
    },
    Asset {
        name: "6135c4139b62e558bb4c755b266f5fa2.data.gz",
        size: 27724487,
        hash: "fa85bfa065f6201466806a5d95e66f9b1162126f59fd13373b9243d1e8c6e474",
    },
];

/// Only constructed after the public build, assets and compiled fields pass their pins.
/// Loading and fetching block; call them off the application event thread.
pub struct VerifiedProfile {
    pub(crate) first_host: String,
    pub(crate) first_port: u16,
    pub(crate) marker: Zeroizing<String>,
    pub(crate) client_version: Zeroizing<Vec<u8>>,
    pub(crate) definitions: Option<std::sync::Arc<crate::definitions::GameDefinitions>>,
}
impl VerifiedProfile {
    pub fn observer_script(&self, binding: &crate::PageBinding) -> Result<crate::ObserverScript> {
        crate::page::script(binding)
    }
    pub fn build(&self) -> &'static str {
        BUILD
    }
    pub fn load_cache(cache: &Path) -> Result<Self> {
        let config = read_bounded(&cache.join("spinel-config.json"), 4096)?;
        let version = read_bounded(&cache.join("version"), 4096)?;
        let (host, port) = deployment(&config, &version)?;
        let page = read_bounded(&cache.join("index.public.html"), 262144)?;
        validate_page(&page)?;
        let mut data = None;
        for asset in &ASSETS {
            let b = read_asset(&cache.join(asset.name), asset)?;
            if asset.name.ends_with(".data.gz") {
                data = Some(b);
            }
        }
        let data = data.ok_or(NetError::BuildChanged)?;
        let metadata = metadata(&data)?;
        verify_hash(
            metadata,
            "7313f719956af84096e41f2afbd39c0cd5812d6ea8ff9b0e9bda9c51b8824ade",
        )?;
        if metadata.get(..8) != Some(&[0xaf, 0x1b, 0xb1, 0xfa, 108, 0, 0, 0]) {
            return Err(NetError::BuildChanged);
        }
        // Full immutable asset fingerprints bind these current-build metadata locators.
        let marker = field(
            metadata,
            480347,
            5,
            "b5a2c96250612366ea272ffac6d9744aaf4b45aacd96aa7cfcb931ee3b558259",
        )?;
        let client_version = field(
            metadata,
            5142552,
            4,
            "e4abdaa9e4fe688b531f6d5257aad881668ae7d84dd3e8fdf305cfcc0e29b9c6",
        )?;
        field(
            metadata,
            127401,
            32,
            "7cd4e6cdb7707127e907a239277913438058d9327d1a184c7bde25e0a4c79b7a",
        )?;
        field(
            metadata,
            452397,
            44,
            "bc25174f10ae444884c9b36c53dd15c38415f007fd04e8c8a0757a766cecb838",
        )?;
        Ok(Self {
            first_host: host,
            first_port: port,
            marker: Zeroizing::new(
                String::from_utf8(marker.to_vec()).map_err(|_| NetError::BuildChanged)?,
            ),
            client_version: Zeroizing::new(client_version.to_vec()),
            definitions: crate::definitions::GameDefinitions::load(cache)?.map(std::sync::Arc::new),
        })
    }
    pub(crate) async fn revalidate_online(&self) -> Result<()> {
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(15))
            .build()
            .map_err(|_| NetError::Transport)?;
        let config = get(&client, &format!("{ORIGIN}/spinel-config.json"), 4096).await?;
        let version = get(&client, &format!("{ORIGIN}/version"), 4096).await?;
        deployment(&config, &version)?;
        let page = get(&client, &format!("{ORIGIN}/"), 262144).await?;
        validate_page(&page)
    }
    /// Fetches only cookie-free public metadata/assets. Redirects are rejected.
    /// The caller supplies an outside-Git cache directory.
    pub fn fetch_public(cache: &Path) -> Result<Self> {
        let mut existing = cache;
        while !existing.exists() {
            existing = existing.parent().ok_or(NetError::Io)?;
        }
        let resolved = existing.canonicalize().map_err(|_| NetError::Io)?;
        if resolved.ancestors().any(|path| path.join(".git").exists()) {
            return Err(NetError::Io);
        }
        let path = cache.to_owned();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|_| NetError::Io)?;
            rt.block_on(fetch(&path))?;
            Self::load_cache(&path)
        })
        .join()
        .map_err(|_| NetError::Io)?
    }
}
pub(crate) fn read_bounded(path: &Path, max: usize) -> Result<Vec<u8>> {
    let file = fs::File::open(path).map_err(|_| NetError::Io)?;
    if file.metadata().map_err(|_| NetError::Io)?.len() > max as u64 {
        return Err(NetError::Capacity);
    }
    let mut out = Vec::new();
    file.take(max as u64 + 1)
        .read_to_end(&mut out)
        .map_err(|_| NetError::Io)?;
    if out.len() > max {
        return Err(NetError::Capacity);
    }
    Ok(out)
}
fn decoded(bytes: &[u8], max: usize) -> Result<Vec<u8>> {
    let out = if bytes.starts_with(&[0x1f, 0x8b]) {
        let mut out = Vec::new();
        GzDecoder::new(bytes)
            .take(max as u64 + 1)
            .read_to_end(&mut out)
            .map_err(|_| NetError::BuildChanged)?;
        out
    } else {
        bytes.to_vec()
    };
    if out.len() > max {
        return Err(NetError::Capacity);
    }
    Ok(out)
}
fn read_asset(path: &Path, asset: &Asset) -> Result<Vec<u8>> {
    let b = decoded(&read_bounded(path, asset.size + 1_048_576)?, asset.size)?;
    validate_asset(&b, asset)?;
    Ok(b)
}
fn validate_asset(b: &[u8], asset: &Asset) -> Result<()> {
    if b.len() != asset.size {
        return Err(NetError::BuildChanged);
    }
    verify_hash(b, asset.hash)
}
pub(crate) fn verify_hash(b: &[u8], hash: &str) -> Result<()> {
    if format!("{:x}", Sha256::digest(b)) == hash {
        Ok(())
    } else {
        Err(NetError::BuildChanged)
    }
}
fn field<'a>(b: &'a [u8], offset: usize, n: usize, hash: &str) -> Result<&'a [u8]> {
    let value = b.get(offset..offset + n).ok_or(NetError::BuildChanged)?;
    verify_hash(value, hash)?;
    Ok(value)
}
fn deployment(config: &[u8], version: &[u8]) -> Result<(String, u16)> {
    verify_hash(
        config,
        "084900013e6edde0ce0eca286dc7ac884a86da2ece1bae361ce0671852092602",
    )?;
    verify_hash(
        version,
        "9ce11c71ef039ffbe6bede54dfcf5b27afb887d44b2a79f57f959638e2e771b0",
    )?;
    let c: serde_json::Value =
        serde_json::from_slice(config).map_err(|_| NetError::BuildChanged)?;
    let v: serde_json::Value =
        serde_json::from_slice(version).map_err(|_| NetError::BuildChanged)?;
    if c["build"] != BUILD || v["build"] != BUILD {
        return Err(NetError::BuildChanged);
    }
    let host = c["firstLogin"]["host"]
        .as_str()
        .ok_or(NetError::BuildChanged)?
        .to_owned();
    let port = c["firstLogin"]["port"]
        .as_u64()
        .and_then(|n| u16::try_from(n).ok())
        .filter(|n| *n > 0)
        .ok_or(NetError::BuildChanged)?;
    Ok((host, port))
}
fn validate_page(page: &[u8]) -> Result<()> {
    let page = std::str::from_utf8(page).map_err(|_| NetError::BuildChanged)?;
    if ASSETS.iter().all(|a| page.contains(a.name)) {
        Ok(())
    } else {
        Err(NetError::BuildChanged)
    }
}
fn metadata(data: &[u8]) -> Result<&[u8]> {
    let mut r = Reader::new(data);
    if r.take(16)? != b"UnityWebData1.0\0" {
        return Err(NetError::BuildChanged);
    }
    let end = r.u32()? as usize;
    if !(20..=data.len()).contains(&end) {
        return Err(NetError::Malformed);
    }
    let mut found = None;
    while r.pos < end {
        let offset = r.u32()? as usize;
        let len = r.u32()? as usize;
        let n = r.u32()? as usize;
        if n > end.saturating_sub(r.pos) {
            return Err(NetError::Malformed);
        }
        let name = r.take(n)?;
        let extent = data
            .get(offset..offset.checked_add(len).ok_or(NetError::Malformed)?)
            .ok_or(NetError::Truncated)?;
        if offset < end {
            return Err(NetError::Malformed);
        }
        if name == b"Il2CppData/Metadata/global-metadata.dat" {
            if found.is_some() || len != MAX_METADATA {
                return Err(NetError::BuildChanged);
            }
            found = Some(extent);
        }
    }
    if r.pos != end {
        return Err(NetError::Malformed);
    }
    found.ok_or(NetError::BuildChanged)
}
async fn get(client: &reqwest::Client, url: &str, limit: usize) -> Result<Vec<u8>> {
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|_| NetError::Transport)?;
    if !response.status().is_success()
        || response.url().as_str() != url
        || response.content_length().is_some_and(|n| n > limit as u64)
    {
        return Err(NetError::BuildChanged);
    }
    let mut body = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(part) = stream.next().await {
        let part = part.map_err(|_| NetError::Transport)?;
        if body.len() + part.len() > limit {
            return Err(NetError::Capacity);
        }
        body.extend_from_slice(&part);
    }
    Ok(body)
}
async fn fetch(cache: &Path) -> Result<()> {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(90))
        .build()
        .map_err(|_| NetError::Transport)?;
    let config = get(&client, &format!("{ORIGIN}/spinel-config.json"), 4096).await?;
    let version = get(&client, &format!("{ORIGIN}/version"), 4096).await?;
    deployment(&config, &version)?;
    let page = get(&client, &format!("{ORIGIN}/"), 262144).await?;
    validate_page(&page)?;
    fs::create_dir_all(cache).map_err(|_| NetError::Io)?;
    for a in &ASSETS {
        if read_asset(&cache.join(a.name), a).is_ok() {
            continue;
        }
        let b = get(
            &client,
            &format!("{ORIGIN}/Build/{}", a.name),
            a.size + 1_048_576,
        )
        .await?;
        let b = decoded(&b, a.size)?;
        validate_asset(&b, a)?;
        atomic_write(cache, a.name, &b)?;
    }
    atomic_write(cache, "spinel-config.json", &config)?;
    atomic_write(cache, "version", &version)?;
    atomic_write(cache, "index.public.html", &page)
}
fn atomic_write(cache: &Path, name: &str, b: &[u8]) -> Result<()> {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let counter = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let pending = cache.join(format!(".{name}.{}.{counter}.tmp", std::process::id()));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&pending)
        .map_err(|_| NetError::Io)?;
    let result = file
        .write_all(b)
        .and_then(|_| file.sync_all())
        .and_then(|_| fs::rename(&pending, cache.join(name)));
    if result.is_err() {
        let _ = fs::remove_file(&pending);
    }
    result.map_err(|_| NetError::Io)
}
pub(crate) fn bridge(host: &str, port: u16) -> Result<Url> {
    if host.is_empty()
        || host.len() > 253
        || port == 0
        || !host
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b".-:".contains(&b))
    {
        return Err(NetError::Malformed);
    }
    let mut url = Url::parse(&format!("{ORIGIN}/net")).map_err(|_| NetError::Malformed)?;
    url.set_scheme("wss").map_err(|_| NetError::Malformed)?;
    url.query_pairs_mut()
        .append_pair("host", host)
        .append_pair("port", &port.to_string())
        .append_pair("b", BUILD)
        .append_pair("r", "1")
        .append_pair("u", "1");
    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn drift_and_unapproved_endpoints_are_rejected() {
        assert_eq!(deployment(b"{}", b"{}"), Err(NetError::BuildChanged));
        assert!(bridge("a&resume=1", 9).is_err());
        assert!(bridge("host", 0).is_err());
        let url = bridge("127.0.0.1", 123).unwrap();
        assert_eq!(url.scheme(), "wss");
        assert_eq!(url.host_str(), Some("tr-play.playall.in.th"));
        assert_eq!(
            validate_page(b"<html>changed</html>"),
            Err(NetError::BuildChanged)
        );
    }
    #[test]
    fn malformed_directory_and_bounded_compression() {
        assert!(metadata(b"not unity data").is_err());
        let mut b = b"UnityWebData1.0\0".to_vec();
        b.extend_from_slice(&21u32.to_le_bytes());
        b.push(0);
        assert!(metadata(&b).is_err());
        assert_eq!(decoded(&[0; 2], 1), Err(NetError::Capacity));
    }
    #[test]
    #[ignore = "requires caller-provided outside-Git public asset cache; no network or account access"]
    fn verified_public_cache() {
        let path = std::env::var_os("OPENTRICKSTER_PUBLIC_CACHE").expect("public cache path");
        let profile = VerifiedProfile::load_cache(Path::new(&path)).unwrap();
        assert_eq!(profile.build(), BUILD);
    }
    #[test]
    #[ignore = "fresh official public HTTP preflight; requires public cache, never credentials or game sockets"]
    fn fresh_public_preflight() {
        let path = std::env::var_os("OPENTRICKSTER_PUBLIC_CACHE").expect("public cache path");
        let profile = VerifiedProfile::load_cache(Path::new(&path)).unwrap();
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(profile.revalidate_online()).unwrap();
    }
}
