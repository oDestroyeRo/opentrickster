use crate::{
    NetError, Result,
    definitions::*,
    dig::Params,
    profile::{ORIGIN, read_bounded, verify_hash},
};
use aes::cipher::{KeyIvInit, StreamCipher};
use base64::{
    Engine,
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
};
use flate2::read::GzDecoder;
use futures_util::StreamExt;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Read,
    path::Path,
};
use zeroize::Zeroizing;

struct PublicFile {
    logical: &'static str,
    size: usize,
    hash: &'static str,
}
const FILES: [PublicFile; 6] = [
    PublicFile {
        logical: "_spinel/item-index-libconfig_thai.bin",
        size: 6254728,
        hash: "1f33cb29efac45471873dc5c6d05320cd5d8a14e03202d91fa1b46ac566c3993",
    },
    PublicFile {
        logical: "libconfig_thai.idx",
        size: 259683,
        hash: "4784253c40e33ca76e6f98430cfa32487e0eb9229a55dc3226af48ed9975d89b",
    },
    PublicFile {
        logical: "libconfig_thai.dat",
        size: 155695673,
        hash: "ab16141a6491bae383f20f56ddbdc88ab0aca317904a33a2a58d8e3b9fa88223",
    },
    PublicFile {
        logical: "gulim13.fnt2",
        size: 567604,
        hash: "a97da1418b88f0920a1f868e735e728fc032070cca422ab9f6f2efb0440f1b3c",
    },
    PublicFile {
        logical: "gulim14.fnt2",
        size: 12789,
        hash: "7d9fa9aa233cab8ec3db58d5e21b0dad3697db1b8bcb0859fe9ab9d169edf468",
    },
    PublicFile {
        logical: "gulim23.fnt2",
        size: 124744,
        hash: "0f5ba9f94371e087a7364a31c34366aa4a6f50530e31b360750a1b85c08d14e9",
    },
];
fn file_name(file: &PublicFile) -> String {
    format!("{}.decoded", file.logical.replace('/', "_"))
}
fn pinned(cache: &Path, file: &PublicFile) -> Result<Vec<u8>> {
    let b = read_bounded(&cache.join(file_name(file)), file.size)?;
    if b.len() != file.size {
        return Err(NetError::BuildChanged);
    }
    verify_hash(&b, file.hash)?;
    Ok(b)
}
pub(crate) fn load(cache: &Path, metadata: &[u8]) -> Result<Option<GameDefinitions>> {
    if !FILES.iter().any(|f| cache.join(file_name(f)).exists()) {
        return Ok(None);
    }
    let public = FILES
        .iter()
        .map(|f| pinned(cache, f))
        .collect::<Result<Vec<_>>>()?;
    let key = metadata
        .get(0x4ed2a8..0x4ed2b8)
        .ok_or(NetError::BuildChanged)?;
    verify_hash(
        key,
        "b5b8c3c831b51613bc8701615db8ec52cda99e4610d0f452a9c984619ba9dea0",
    )?;
    let effects = metadata
        .get(0x4d09c8..0x4d09e0)
        .ok_or(NetError::BuildChanged)?;
    verify_hash(
        effects,
        "65a96e5111fde179295e050214fd70276da0446bb6fec006bfc2bc87b759efc3",
    )?;
    let slots = metadata
        .get(0x4d16d0..0x4d16e0)
        .ok_or(NetError::BuildChanged)?;
    verify_hash(
        slots,
        "f79067ab1dc0e54d3c4b8d6c007d3a06f9ba33296ce8d0e6e038b335115bf800",
    )?;
    let entries = index(&public[1])?;
    stamp(&public[0], &entries, public[2].len(), public[1].len())?;
    if public[2].first() != Some(&1) {
        return Err(NetError::BuildChanged);
    }
    let (items, labels) = items(&public[0])?;
    let mut drills = BTreeMap::new();
    let mut drill_meta = BTreeMap::new();
    for row in table(
        &public[2],
        &entries,
        key,
        "Item_DrillParam",
        191,
        "f73ce98fb594e8189b94fb97d3353ac6dc4ab7bd94c16d803a0cdec8c84a7bbd",
    )? {
        let id = u32::try_from(cell(&row, "Id")?).map_err(|_| NetError::Malformed)?;
        drills.entry(id).or_insert(Params {
            area: cell(&row, "BarEffArea")? as i16 as i32,
            speed: cell(&row, "BarSpeed")? as i16 as i32,
            mass: cell(&row, "ScoopMass")? as i16 as i32,
            limit: cell(&row, "DigDepth")? as i16 as i32,
            nature: cell(&row, "GroundNature")? as u32,
        });
        drill_meta.entry(id).or_insert(DrillMeta {
            grade: cell(&row, "Grade")? as i16 as i32,
            maximum_life: cell(&row, "DrillLife")? as i16 as i32,
        });
    }
    let mut gun_params = BTreeMap::new();
    for row in table(
        &public[2],
        &entries,
        key,
        "Item_GunParam2",
        613,
        "c897abd5de36dd24c873009efd79710760eef7206eeff9008613dabf9d078af8",
    )? {
        let id = u32::try_from(cell(&row, "ID")?).map_err(|_| NetError::Malformed)?;
        gun_params.entry(id).or_insert(GunParams {
            kind: cell(&row, "GunType")? as i32,
            speed: cell(&row, "GaugeSpeed")? as i32,
            range: cell(&row, "PowRange")? as i32,
            start: cell(&row, "PowStartPoint")? as i32,
            min_range: cell(&row, "MinAttackRange")? as i32,
            max_range: cell(&row, "MaxAttackRange")? as i32,
        });
    }
    let mut zones = BTreeMap::new();
    let mut zone_labels = BTreeMap::new();
    for row in table(
        &public[2],
        &entries,
        key,
        "MapInfoEx",
        696,
        "f2d89e609d624ee0e367987ee9719c07ab358e0f232c00096dea6223a0c70820",
    )? {
        let id = u16::try_from(cell(&row, "ID")?).map_err(|_| NetError::Malformed)?;
        zones
            .entry(id)
            .or_insert((cell(&row, "Type")? as i32, cell(&row, "SubType")? as i32));
        zone_labels
            .entry(id)
            .or_insert(row.get("Name").ok_or(NetError::Malformed)?.clone());
    }
    // The immutable Unity data pin binds #27's relevant-overlay absence proof.
    // Any new Unity/public asset fingerprint is refused before this resolver runs.
    for (n, b) in public.iter().enumerate().skip(3) {
        let values = std::str::from_utf8(b)
            .map_err(|_| NetError::Malformed)?
            .split_whitespace()
            .map(|s| s.parse::<f64>().map_err(|_| NetError::Malformed))
            .collect::<Result<Vec<_>>>()?;
        if values.len() != if n == 4 { 1600 } else { 35964 }
            || values.iter().any(|v| !v.is_finite())
        {
            return Err(NetError::Malformed);
        }
    }
    if items.len() != 18118 || drills.len() != 191 || gun_params.len() != 611 || zones.len() != 696
    {
        return Err(NetError::BuildChanged);
    }
    Ok(Some(GameDefinitions {
        item_types: items
            .iter()
            .map(|(&id, i)| (id, (i.kind, i.subtype)))
            .collect(),
        items,
        labels,
        drills,
        drill_meta,
        gun_params,
        zones,
        zone_labels,
        gun_slots: slots
            .as_chunks::<4>()
            .0
            .iter()
            .map(|b| i32::from_le_bytes(*b) as usize)
            .collect(),
        normal_form_effects: effects
            .as_chunks::<4>()
            .0
            .iter()
            .map(|b| i32::from_le_bytes(*b))
            .collect(),
    }))
}

struct IndexEntry {
    name: String,
    start: usize,
}
fn index(b: &[u8]) -> Result<Vec<IndexEntry>> {
    let mut r = crate::wire::Reader::new(b);
    let mut entries = Vec::new();
    while r.remaining() > 0 {
        if entries.len() >= 16384 {
            return Err(NetError::Capacity);
        }
        let rest = &b[r.pos..];
        let len = rest
            .iter()
            .position(|n| *n == 0)
            .filter(|n| *n <= 256)
            .ok_or(NetError::Malformed)?;
        let name = std::str::from_utf8(r.take(len)?)
            .map_err(|_| NetError::Malformed)?
            .to_owned();
        r.u8()?;
        let start = r.u32()? as usize;
        if entries
            .last()
            .is_some_and(|e: &IndexEntry| e.start >= start)
        {
            return Err(NetError::Malformed);
        }
        entries.push(IndexEntry { name, start });
    }
    Ok(entries)
}
struct Stamp(u64);
impl Stamp {
    fn new() -> Self {
        Self(14695981039346656037)
    }
    fn add(&mut self, v: u64) {
        let n = (self.0 ^ v).wrapping_mul(1099511628211);
        self.0 = n ^ (n >> 29);
    }
    fn string(&mut self, s: &str) {
        self.add(s.len() as u64);
        let (chunks, remainder) = s.as_bytes().as_chunks::<8>();
        for b in chunks {
            self.add(u64::from_le_bytes(*b));
        }
        for &b in remainder {
            self.add(b as u64);
        }
    }
}
fn stamp(b: &[u8], entries: &[IndexEntry], data_len: usize, index_len: usize) -> Result<()> {
    let mut r = crate::wire::Reader::new(b);
    if r.take(8)? != b"IIDX\x01\0\0\0" {
        return Err(NetError::BuildChanged);
    }
    r.u64()?;
    let overlay = r.u64()?;
    let runtime = r.u64()?;
    let data_size = r.u64()?;
    r.u64()?;
    let index_size = r.u64()?;
    r.u64()?;
    let count = r.u32()? as usize;
    if count > 10000 {
        return Err(NetError::Capacity);
    }
    let mut reads = BTreeSet::new();
    for _ in 0..count {
        if !reads.insert(r.u64()?) {
            return Err(NetError::Malformed);
        }
    }
    let mut expected = BTreeSet::new();
    for name in ["ItemParamCM2", "ItemParamEx2", "ItemParamEx", "ItemParam2"] {
        let mut h = Stamp::new();
        h.add(1);
        h.string(name);
        expected.insert(h.0);
    }
    let mut h = Stamp::new();
    h.add(1);
    h.add(entries.len() as u64);
    for (i, e) in entries.iter().enumerate() {
        let end = entries
            .get(i + 1)
            .map_or(data_len.checked_sub(1).ok_or(NetError::Malformed)?, |e| {
                e.start
            });
        if e.start >= end || end >= data_len {
            return Err(NetError::Malformed);
        }
        h.string(&e.name);
        h.add(e.start as u64);
        h.add(end as u64);
    }
    let mut empty = Stamp::new();
    empty.add(1);
    if reads != expected
        || overlay != empty.0
        || runtime != h.0
        || data_size != data_len as u64
        || index_size != index_len as u64
    {
        return Err(NetError::BuildChanged);
    }
    Ok(())
}

type Row = BTreeMap<String, String>;
fn cell(row: &Row, name: &str) -> Result<i64> {
    let value = row
        .get(name)
        .filter(|s| !s.trim().is_empty())
        .ok_or(NetError::Malformed)?
        .trim()
        .parse::<f64>()
        .map_err(|_| NetError::Malformed)?;
    if !value.is_finite() {
        return Err(NetError::Malformed);
    }
    Ok(value.trunc() as i64)
}
fn xtea(cipher: &[u8], key: &[u8]) -> Result<Vec<u8>> {
    if !cipher.len().is_multiple_of(8) || key.len() != 16 {
        return Err(NetError::Malformed);
    }
    let key = Zeroizing::new([
        u32::from_le_bytes(key[0..4].try_into().unwrap()),
        u32::from_le_bytes(key[4..8].try_into().unwrap()),
        u32::from_le_bytes(key[8..12].try_into().unwrap()),
        u32::from_le_bytes(key[12..16].try_into().unwrap()),
    ]);
    let mut out = Vec::with_capacity(cipher.len());
    for block in cipher.as_chunks::<8>().0 {
        let mut a = u32::from_be_bytes(block[..4].try_into().unwrap());
        let mut b = u32::from_be_bytes(block[4..].try_into().unwrap());
        let mut sum = 0xc6ef3720u32;
        for _ in 0..32 {
            b = b.wrapping_sub(
                ((a << 4) ^ (a >> 5)).wrapping_add(a)
                    ^ sum.wrapping_add(key[((sum >> 11) & 3) as usize]),
            );
            sum = sum.wrapping_sub(0x9e3779b9);
            a = a.wrapping_sub(
                ((b << 4) ^ (b >> 5)).wrapping_add(b) ^ sum.wrapping_add(key[(sum & 3) as usize]),
            );
        }
        out.extend_from_slice(&a.to_be_bytes());
        out.extend_from_slice(&b.to_be_bytes());
    }
    Ok(out)
}
fn table(
    data: &[u8],
    entries: &[IndexEntry],
    key: &[u8],
    name: &str,
    count: usize,
    hash: &str,
) -> Result<Vec<Row>> {
    let i = entries
        .iter()
        .position(|e| e.name == name)
        .ok_or(NetError::BuildChanged)?;
    let end = entries.get(i + 1).ok_or(NetError::Malformed)?.start;
    let cipher = data
        .get(entries[i].start..end)
        .filter(|b| b.len() <= 8 * 1024 * 1024)
        .ok_or(NetError::Capacity)?;
    let raw = xtea(cipher, key)?;
    let mut raw: Vec<u8> = raw.into_iter().filter(|b| *b != 0).collect();
    let end = raw
        .windows(8)
        .position(|b| b == b"</TABLE>")
        .ok_or(NetError::Malformed)?
        + 8;
    raw.truncate(end);
    verify_hash(&raw, hash)?;
    let xml = std::str::from_utf8(&raw).map_err(|_| NetError::Malformed)?;
    let doc = roxmltree::Document::parse_with_options(
        xml,
        roxmltree::ParsingOptions {
            allow_dtd: false,
            nodes_limit: 300000,
        },
    )
    .map_err(|_| NetError::Malformed)?;
    let root = doc.root_element();
    if root.tag_name().name() != "TABLE"
        || root.attribute("name") != Some(name)
        || root
            .attribute("RowCount")
            .and_then(|n| n.parse::<usize>().ok())
            != Some(count)
    {
        return Err(NetError::BuildChanged);
    }
    let rows = root
        .children()
        .filter(|n| n.has_tag_name("ROW"))
        .map(|row| {
            row.children()
                .filter(|n| n.is_element())
                .map(|n| {
                    (
                        n.tag_name().name().to_owned(),
                        n.descendants()
                            .filter(|n| n.is_text())
                            .filter_map(|n| n.text())
                            .collect::<String>(),
                    )
                })
                .collect::<Row>()
        })
        .collect::<Vec<_>>();
    if rows.len() != count {
        return Err(NetError::BuildChanged);
    }
    Ok(rows)
}

struct ItemReader<'a> {
    r: crate::wire::Reader<'a>,
}
impl ItemReader<'_> {
    fn unsigned(&mut self) -> Result<u64> {
        let mut n = 0u64;
        for i in 0..10 {
            let b = self.r.u8()?;
            if i == 9 && b > 1 {
                return Err(NetError::Malformed);
            }
            n |= ((b & 127) as u64) << (i * 7);
            if b < 128 {
                return Ok(n);
            }
        }
        Err(NetError::Malformed)
    }
    fn signed(&mut self) -> Result<i64> {
        let n = self.unsigned()?;
        Ok(((n >> 1) as i64) ^ -((n & 1) as i64))
    }
    fn int(&mut self) -> Result<i32> {
        i32::try_from(self.signed()?).map_err(|_| NetError::Malformed)
    }
    fn number(&mut self) -> Result<f64> {
        let n = self.signed()?;
        let v = if n & 1 == 0 {
            (n / 2) as f64
        } else if n == 1 {
            f64::from_le_bytes(self.r.take(8)?.try_into().unwrap())
        } else {
            return Err(NetError::Malformed);
        };
        if !v.is_finite() {
            return Err(NetError::Malformed);
        }
        Ok(v)
    }
    fn string(&mut self) -> Result<String> {
        let len = usize::try_from(self.unsigned()?).map_err(|_| NetError::Capacity)?;
        if len > 65536 {
            return Err(NetError::Capacity);
        }
        std::str::from_utf8(self.r.take(len)?)
            .map(str::to_owned)
            .map_err(|_| NetError::Malformed)
    }
    fn string_ref(&mut self, strings: &[Option<String>]) -> Result<Option<String>> {
        let i = usize::try_from(self.int()?).map_err(|_| NetError::Malformed)?;
        strings.get(i).cloned().ok_or(NetError::Malformed)
    }
}
fn items(b: &[u8]) -> Result<(BTreeMap<u32, ItemInfo>, BTreeMap<u32, String>)> {
    let mut r = ItemReader {
        r: crate::wire::Reader::new(b),
    };
    if r.r.take(8)? != b"IIDX\x01\0\0\0" {
        return Err(NetError::BuildChanged);
    }
    r.r.take(56)?;
    let reads = r.r.u32()? as usize;
    if reads > 10000 {
        return Err(NetError::Capacity);
    }
    r.r.take(reads * 8)?;
    let count = usize::try_from(r.int()?).map_err(|_| NetError::Malformed)?;
    if count > 100000 {
        return Err(NetError::Capacity);
    }
    let mut strings = vec![None];
    for _ in 0..count {
        strings.push(Some(r.string()?));
    }
    let count = r.int()?;
    if count != 18118 {
        return Err(NetError::BuildChanged);
    }
    let mut items = BTreeMap::new();
    let mut labels = BTreeMap::new();
    for _ in 0..count {
        let id = u32::try_from(r.signed()?).map_err(|_| NetError::Malformed)?;
        let name = r.string_ref(&strings)?.ok_or(NetError::Malformed)?;
        for _ in 0..7 {
            r.string_ref(&strings)?;
        }
        let class = r.int()?;
        let kind = r.int()?;
        let subtype = r.int()?;
        for _ in 0..3 {
            r.int()?;
        }
        let flags = r.int()?;
        let flags_ex = r.int()?;
        r.int()?;
        let weight = r.signed()?;
        r.signed()?;
        let min_level = r.int()?;
        let hp = r.signed()?;
        let mp = r.signed()?;
        for _ in 0..6 {
            r.int()?;
        }
        let exist_type = r.int()?;
        let effect = r.int()?;
        let life = r.int()?;
        let ftype = r.int()?;
        for _ in 0..3 {
            r.int()?;
        }
        for _ in 0..3 {
            r.signed()?;
        }
        let delay = r.number()?;
        r.number()?;
        r.number()?;
        let hp_rate = r.number()?;
        let mp_rate = r.number()?;
        let stats = r.int()?;
        if !(-1..1000).contains(&stats) {
            return Err(NetError::Malformed);
        }
        for _ in 0..stats.max(0) {
            r.number()?;
        }
        if items
            .insert(
                id,
                ItemInfo {
                    weight,
                    class,
                    kind,
                    subtype,
                    flags,
                    flags_ex,
                    min_level,
                    hp,
                    mp,
                    hp_rate,
                    mp_rate,
                    delay,
                    effect,
                    exist_type,
                    life,
                    ftype,
                },
            )
            .is_some()
        {
            return Err(NetError::Malformed);
        }
        labels.insert(id, name);
    }
    r.r.end()?;
    Ok((items, labels))
}

fn crypt(bytes: &mut [u8], key: &[u8], iv: &[u8]) -> Result<()> {
    let mut cipher =
        ctr::Ctr128BE::<aes::Aes256>::new_from_slices(key, iv).map_err(|_| NetError::Malformed)?;
    cipher
        .try_apply_keystream(bytes)
        .map_err(|_| NetError::Capacity)
}
pub(crate) async fn fetch(client: &reqwest::Client, cache: &Path) -> Result<()> {
    let config = crate::profile::get(
        client,
        &format!("{ORIGIN}/pack/_spinel/data-server.json"),
        4096,
    )
    .await?;
    let config: serde_json::Value =
        serde_json::from_slice(&config).map_err(|_| NetError::Malformed)?;
    if config
        != serde_json::json!({"rangeQuery":true,"encrypted":true,"dataVersion":"KIcgbcTZu7eh"})
    {
        return Err(NetError::BuildChanged);
    }
    let keydoc = Zeroizing::new(
        crate::profile::get(client, &format!("{ORIGIN}/pack/_spinel/data-key"), 4096).await?,
    );
    verify_hash(
        &keydoc,
        "6c06ccb0e9f57269b34a1fecb68566e6ed4754ae56e5d190a14b5358b623b19e",
    )?;
    let mut value: serde_json::Value =
        serde_json::from_slice(&keydoc).map_err(|_| NetError::Malformed)?;
    let text = Zeroizing::new(
        value
            .get_mut("key")
            .and_then(|v| v.as_str().map(str::to_owned))
            .ok_or(NetError::Malformed)?,
    );
    if let Some(serde_json::Value::String(s)) = value.get_mut("key") {
        use zeroize::Zeroize;
        s.zeroize();
    }
    let key = Zeroizing::new(
        STANDARD
            .decode(text.as_bytes())
            .map_err(|_| NetError::Malformed)?,
    );
    if key.len() != 32 {
        return Err(NetError::BuildChanged);
    }
    for file in &FILES {
        if pinned(cache, file).is_ok() {
            continue;
        }
        let mut mac = Hmac::<Sha256>::new_from_slice(&key).map_err(|_| NetError::Malformed)?;
        mac.update(b"spinel-data-name\0");
        mac.update(file.logical.as_bytes());
        let digest = mac.finalize().into_bytes();
        let nonce = &digest[..16];
        let mut name = file.logical.as_bytes().to_vec();
        crypt(&mut name, &key, nonce)?;
        let mut token = nonce.to_vec();
        token.extend(name);
        let url = format!(
            "{ORIGIN}/pack/{}.bin?v=KIcgbcTZu7eh",
            URL_SAFE_NO_PAD.encode(token)
        );
        let response = client
            .get(&url)
            .send()
            .await
            .map_err(|_| NetError::Transport)?;
        if !response.status().is_success()
            || response.url().as_str() != url
            || response
                .headers()
                .get("x-spinel-enc")
                .and_then(|v| v.to_str().ok())
                != Some("aes-256-ctr")
            || response
                .headers()
                .get("x-spinel-coding")
                .and_then(|v| v.to_str().ok())
                != Some("gzip")
        {
            return Err(NetError::BuildChanged);
        }
        let iv = STANDARD
            .decode(
                response
                    .headers()
                    .get("x-spinel-iv")
                    .ok_or(NetError::Malformed)?,
            )
            .map_err(|_| NetError::Malformed)?;
        if iv.len() != 16 {
            return Err(NetError::Malformed);
        }
        let limit = file.size + 1048576;
        if response.content_length().is_some_and(|n| n > limit as u64) {
            return Err(NetError::Capacity);
        }
        let mut bytes = Vec::new();
        let mut stream = response.bytes_stream();
        while let Some(part) = stream.next().await {
            let part = part.map_err(|_| NetError::Transport)?;
            if bytes.len() + part.len() > limit {
                return Err(NetError::Capacity);
            }
            bytes.extend_from_slice(&part);
        }
        crypt(&mut bytes, &key, &iv)?;
        let mut decoded = Vec::new();
        GzDecoder::new(bytes.as_slice())
            .take(file.size as u64 + 1)
            .read_to_end(&mut decoded)
            .map_err(|_| NetError::Malformed)?;
        if decoded.len() != file.size {
            return Err(NetError::BuildChanged);
        }
        verify_hash(&decoded, file.hash)?;
        crate::profile::atomic_write(cache, &file_name(file), &decoded)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn malformed_indexes_numbers_and_xtea_alignment_fail_closed() {
        assert!(index(b"abc").is_err());
        assert!(index(b"a\0\x02\0\0\0b\0\x01\0\0\0").is_err());
        assert!(xtea(&[0; 7], &[0; 16]).is_err());
        let plain = xtea(&[0xde, 0xe9, 0xd4, 0xd8, 0xf7, 0x13, 0x1e, 0xd9], &[0; 16]).unwrap();
        assert_eq!(plain, vec![0; 8]);
        let mut r = ItemReader {
            r: crate::wire::Reader::new(&[0xff; 10]),
        };
        assert!(r.unsigned().is_err());
        let mut r = ItemReader {
            r: crate::wire::Reader::new(&[6]),
        };
        assert!(r.number().is_err());
        assert!(cell(&BTreeMap::from([("x".into(), "NaN".into())]), "x").is_err());
    }
    #[test]
    #[ignore = "requires caller-provided pinned public caches; no network, account or target execution"]
    fn verified_public_catalog() {
        let path = std::env::var_os("OPENTRICKSTER_PUBLIC_CACHE").expect("public cache");
        let profile = crate::VerifiedProfile::load_cache(Path::new(&path)).unwrap();
        let data = profile.definitions.unwrap();
        assert_eq!(data.items.len(), 18118);
        assert_eq!(data.gun_params.len(), 611);
        assert_eq!(data.gun_params[&3580].speed, 120);
        assert_eq!(data.gun_params[&3580].max_range, 245);
        assert_eq!(data.drills[&8001].area, 70);
        assert_eq!(data.drill_meta[&8001].maximum_life, 300);
        assert_eq!(data.gun_slots, [4, 9, 1, 22]);
        assert_eq!(data.items[&2001].hp, 200);
        assert!(!data.items.contains_key(&19907));
    }
}
