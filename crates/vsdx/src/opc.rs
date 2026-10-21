//! Open Packaging Conventions (OPC) container access over a ZIP file.

use std::collections::HashMap;
use std::io::{Read, Seek};
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum OpcError {
    #[error("I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("zip: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("missing required part: {0}")]
    MissingPart(String),
    #[error("bad relationship XML in {0}")]
    BadRels(String),
}

/// A parsed OPC package: part bytes plus the content-type map.
#[derive(Debug, Clone)]
pub struct Package {
    parts: HashMap<String, Vec<u8>>,
    /// extension ("png", "rels", …) -> content type
    defaults: HashMap<String, String>,
    /// part name (leading '/') -> content type override
    overrides: HashMap<String, String>,
}

impl Package {
    pub fn open(path: &Path) -> Result<Self, OpcError> {
        Self::from_reader(std::fs::File::open(path)?)
    }

    /// Parse a package from any seekable stream (in-memory templates).
    pub fn from_reader(reader: impl Read + Seek) -> Result<Self, OpcError> {
        let mut archive = zip::ZipArchive::new(reader)?;
        let mut parts = HashMap::new();
        for name in archive.file_names().map(str::to_owned).collect::<Vec<_>>() {
            let mut f = archive.by_name(&name)?;
            let mut buf = Vec::new();
            f.read_to_end(&mut buf)?;
            parts.insert(name.trim_start_matches('/').to_string(), buf);
        }
        let ct = parts
            .get("[Content_Types].xml")
            .ok_or_else(|| OpcError::MissingPart("[Content_Types].xml".into()))?;
        let (defaults, overrides) = parse_content_types(&String::from_utf8_lossy(ct))?;
        Ok(Self { parts, defaults, overrides })
    }

    pub fn part_names(&self) -> Vec<&str> {
        let mut v: Vec<&str> = self.parts.keys().map(String::as_str).collect();
        v.sort();
        v
    }

    pub fn part(&self, name: &str) -> Option<&[u8]> {
        self.parts
            .get(name.trim_start_matches('/'))
            .map(Vec::as_slice)
    }

    pub fn content_type_of(&self, part: &str) -> Option<String> {
        let part = part.trim_start_matches('/');
        if let Some(t) = self.overrides.get(part) {
            return Some(t.clone());
        }
        part.rsplit('.').next().and_then(|ext| self.defaults.get(ext).cloned())
    }

    /// Relationships declared for `part` (looks up `<dir>/_rels/<name>.rels`).
    /// Returns (Id, Type, absolute target part name) triples.
    pub fn relationships(&self, part: &str) -> Vec<(String, String, String)> {
        let part = part.trim_start_matches('/');
        let (dir, file) = match part.rfind('/') {
            Some(i) => (part[..i].to_string(), &part[i + 1..]),
            None => (String::new(), part),
        };
        let rels_name = if dir.is_empty() {
            format!("_rels/{file}.rels")
        } else {
            format!("{dir}/_rels/{file}.rels")
        };
        let Some(raw) = self.part(&rels_name) else {
            return Vec::new();
        };
        parse_relationships(&String::from_utf8_lossy(raw), &dir)
    }

    /// Resolve the target of a relationship relative to the declaring part.
    pub fn rel_target(&self, part: &str, rel_type_suffix: &str) -> Option<String> {
        self.relationships(part)
            .into_iter()
            .find(|(_, ty, _)| ty.rsplit('/').next() == Some(rel_type_suffix))
            .map(|(_, _, target)| target)
    }
}

/// Normalize `base_dir` + target (which may contain `../`) to a package part name.
fn resolve_relative(base_dir: &str, target: &str) -> String {
    let target = target.trim_start_matches('/');
    let mut stack: Vec<&str> = if base_dir.is_empty() {
        Vec::new()
    } else {
        base_dir.split('/').collect()
    };
    for seg in target.split('/') {
        match seg {
            "." | "" => {}
            ".." => {
                stack.pop();
            }
            s => stack.push(s),
        }
    }
    stack.join("/")
}

type TypeMaps = (HashMap<String, String>, HashMap<String, String>);

fn parse_content_types(xml: &str) -> Result<TypeMaps, OpcError> {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let mut defaults = HashMap::new();
    let mut overrides = HashMap::new();
    let mut r = Reader::from_str(xml);
    let mut buf = Vec::new();
    loop {
        match r.read_event_into(&mut buf) {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                if e.name().as_ref() == "Default" || e.name().as_ref() == "Override" {
                    let mut ext = None;
                    let mut ct: Option<String> = None;
                    let mut pn = None;
                    for a in e.attributes().flatten() {
                        match a.key.as_ref() {
                            "Extension" => ext = Some(a.normalized_value(quick_xml::XmlVersion::Implicit1_0).unwrap_or_default().into_owned()),
                            "ContentType" => ct = Some(a.normalized_value(quick_xml::XmlVersion::Implicit1_0).unwrap_or_default().into_owned()),
                            "PartName" => pn = Some(a.normalized_value(quick_xml::XmlVersion::Implicit1_0).unwrap_or_default().into_owned()),
                            _ => {}
                        }
                    }
                    if ext.is_some() {
                        if let (Some(ext), Some(ct)) = (ext, ct) {
                            defaults.insert(ext.to_ascii_lowercase(), ct);
                        }
                    } else if let (Some(pn), Some(ct)) = (pn, ct) {
                        overrides.insert(pn.trim_start_matches('/').to_string(), ct);
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(OpcError::BadRels(format!("content types: {e}"))),
            _ => {}
        }
        buf.clear();
    }
    Ok((defaults, overrides))
}

fn parse_relationships(xml: &str, base_dir: &str) -> Vec<(String, String, String)> {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let mut out = Vec::new();
    let mut r = Reader::from_str(xml);
    let mut buf = Vec::new();
    loop {
        match r.read_event_into(&mut buf) {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let is_rel = e.name().as_ref() == "Relationship";
                if is_rel {
                    let mut id = None;
                    let mut ty = None;
                    let mut target = None;
                    for a in e.attributes().flatten() {
                        match a.key.as_ref() {
                            "Id" => id = a
                                .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                                .ok()
                                .map(|v| v.into_owned()),
                            "Type" => ty = a
                                .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                                .ok()
                                .map(|v| v.into_owned()),
                            "Target" => target = a
                                .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                                .ok()
                                .map(|v| v.into_owned()),
                            _ => {}
                        }
                    }
                    if let (Some(id), Some(ty), Some(target)) = (id, ty, target) {
                        // External targets (images on the web) are kept as-is.
                        let abs = if ty.contains("/external") {
                            target.to_string()
                        } else {
                            resolve_relative(base_dir, &target)
                        };
                        out.push((id, ty, abs));
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    out
}

/// Minimal XML attribute-value unescape: the five predefined entities plus
/// decimal/hex numeric character references (`&#65;`, `&#x41;`).
pub(crate) fn unescape(raw: &[u8]) -> String {
    let s = String::from_utf8_lossy(raw);
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '&' {
            let mut ent = String::new();
            for c2 in chars.by_ref() {
                if c2 == ';' {
                    break;
                }
                ent.push(c2);
                if ent.len() > 10 {
                    break;
                }
            }
            let resolved = match ent.as_str() {
                "amp" => "&".to_string(),
                "lt" => "<".to_string(),
                "gt" => ">".to_string(),
                "quot" => "\"".to_string(),
                "apos" => "'".to_string(),
                _ => {
                    let code = ent
                        .strip_prefix("#x")
                        .or_else(|| ent.strip_prefix("#X"))
                        .and_then(|h| u32::from_str_radix(h, 16).ok())
                        .or_else(|| ent.strip_prefix('#').and_then(|d| d.parse::<u32>().ok()));
                    match code.and_then(char::from_u32) {
                        Some(c) => c.to_string(),
                        // Unknown entity: reconstruct it verbatim.
                        None => format!("&{ent};"),
                    }
                }
            };
            out.push_str(&resolved);
        } else {
            out.push(c);
        }
    }
    out
}
