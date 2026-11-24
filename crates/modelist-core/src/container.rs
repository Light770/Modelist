//! The `.modelist` project file: a ZIP container of JSON parts.
//!
//! No open standard stores a parametric, multi-garment pattern session, so the format combines
//! existing ones (see the format doc and `docs/format/modelist-v1.schema.json`):
//!
//! ```text
//! mimetype                          "application/vnd.modelist+zip", stored uncompressed, first
//! manifest.json                     format version, writer, every part with media type + sha256
//! project.json                      body, size table, grading, garment order, active garment
//! components/<id>/component.json    one garment: template, ease, formulas, fabrics, ops, pieces
//! thumbnails/preview.png            optional preview for file browsers and the library
//! session.json                      optional UI state (ignored by other tools)
//! ```
//!
//! Like OpenDocument and EPUB, the first entry names the format so tools can identify a file
//! from its first bytes. Parts this build does not know (written by a newer build, or trims data
//! from the trims library) are read back as [`ContainerPart`]s so they can be written again
//! unchanged. Version-0 `.modelist.json` files keep opening through [`PatternProject::from_json`].

use crate::model::{
    BodyMeasurements, GarmentComponent, GradeMode, PatternProject, SizeTable, StackAnchor,
    PROJECT_FORMAT_VERSION,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{Cursor, Read, Write};
use uuid::Uuid;
use zip::write::SimpleFileOptions;
use zip::CompressionMethod;

/// Media type of a `.modelist` file (also the content of its `mimetype` entry).
pub const MIMETYPE: &str = "application/vnd.modelist+zip";
const MANIFEST: &str = "manifest.json";
const PROJECT: &str = "project.json";
const THUMBNAIL: &str = "thumbnails/preview.png";
const SESSION: &str = "session.json";

/// A part of the container that this build does not interpret, kept byte for byte.
#[derive(Debug, Clone, PartialEq)]
pub struct ContainerPart {
    pub path: String,
    pub media_type: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ManifestPart {
    pub path: String,
    pub media_type: String,
    /// Lower-case hex SHA-256 of the part's bytes.
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Writer {
    pub app: String,
    pub version: String,
}

/// `manifest.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Manifest {
    /// Always `"modelist"`.
    pub format: String,
    pub format_version: u32,
    pub writer: Writer,
    /// When the file was written (ISO 8601), as given by the writing app.
    #[serde(default)]
    pub modified: Option<String>,
    /// Every part except `mimetype` and the manifest itself.
    pub parts: Vec<ManifestPart>,
}

/// Where a garment of the project is stored.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ComponentRef {
    pub id: Uuid,
    pub path: String,
}

/// `project.json`: the project without its garments, which have one part each.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ProjectDoc {
    pub format_version: u32,
    pub id: Uuid,
    pub name: String,
    pub measurements: BodyMeasurements,
    pub size_table: SizeTable,
    pub grade_mode: GradeMode,
    pub stack_anchor: StackAnchor,
    /// The garments, in project order.
    pub components: Vec<ComponentRef>,
    #[serde(default)]
    pub active_component: usize,
}

/// Everything read from a `.modelist` file.
#[derive(Debug, Clone)]
pub struct OpenedContainer {
    pub project: PatternProject,
    pub manifest: Manifest,
    pub session_json: Option<String>,
    pub thumbnail_png: Option<Vec<u8>>,
    /// Parts this build does not interpret; pass them back to [`write_container`] on save.
    pub extra_parts: Vec<ContainerPart>,
}

/// What [`write_container`] writes besides the project.
#[derive(Debug, Clone, Default)]
pub struct WriteOptions<'a> {
    pub writer_version: &'a str,
    /// ISO 8601 time of the save, from the caller (the kernel has no clock in WASM).
    pub modified: Option<&'a str>,
    pub session_json: Option<&'a str>,
    pub thumbnail_png: Option<&'a [u8]>,
    pub extra_parts: &'a [ContainerPart],
}

/// True when `bytes` start like a ZIP file (a `.modelist` container rather than plain JSON).
pub fn is_container(bytes: &[u8]) -> bool {
    bytes.starts_with(b"PK\x03\x04")
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

fn component_path(id: Uuid) -> String {
    format!("components/{id}/component.json")
}

/// Writes a project as a `.modelist` container.
pub fn write_container(project: &PatternProject, opts: &WriteOptions) -> Result<Vec<u8>, String> {
    let err = |e: zip::result::ZipError| e.to_string();
    let io = |e: std::io::Error| e.to_string();
    let mut parts: Vec<(String, String, Vec<u8>)> = Vec::new();
    let doc = ProjectDoc {
        format_version: PROJECT_FORMAT_VERSION,
        id: project.id,
        name: project.name.clone(),
        measurements: project.measurements.clone(),
        size_table: project.size_table.clone(),
        grade_mode: project.grade_mode,
        stack_anchor: project.stack_anchor,
        components: project
            .components
            .iter()
            .map(|c| ComponentRef { id: c.id, path: component_path(c.id) })
            .collect(),
        active_component: project.active_component,
    };
    let json = |v: &dyn erased::Json| v.pretty().map_err(|e| e.to_string());
    parts.push((PROJECT.into(), "application/json".into(), json(&doc)?.into_bytes()));
    for c in &project.components {
        parts.push((component_path(c.id), "application/json".into(), json(c)?.into_bytes()));
    }
    if let Some(png) = opts.thumbnail_png {
        parts.push((THUMBNAIL.into(), "image/png".into(), png.to_vec()));
    }
    if let Some(session) = opts.session_json {
        parts.push((SESSION.into(), "application/json".into(), session.as_bytes().to_vec()));
    }
    for extra in opts.extra_parts {
        if !parts.iter().any(|(p, _, _)| *p == extra.path) && extra.path != MANIFEST && extra.path != "mimetype" {
            parts.push((extra.path.clone(), extra.media_type.clone(), extra.bytes.clone()));
        }
    }
    let manifest = Manifest {
        format: "modelist".into(),
        format_version: PROJECT_FORMAT_VERSION,
        writer: Writer { app: "Modelist".into(), version: opts.writer_version.into() },
        modified: opts.modified.map(String::from),
        parts: parts
            .iter()
            .map(|(path, media_type, bytes)| ManifestPart {
                path: path.clone(),
                media_type: media_type.clone(),
                sha256: sha256_hex(bytes),
            })
            .collect(),
    };

    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    let deflated = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    zip.start_file("mimetype", stored).map_err(err)?;
    zip.write_all(MIMETYPE.as_bytes()).map_err(io)?;
    zip.start_file(MANIFEST, deflated).map_err(err)?;
    zip.write_all(json(&manifest)?.as_bytes()).map_err(io)?;
    for (path, media_type, bytes) in &parts {
        // Already-compressed images are stored as they are.
        let opt = if media_type.starts_with("image/") { stored } else { deflated };
        zip.start_file(path.as_str(), opt).map_err(err)?;
        zip.write_all(bytes).map_err(io)?;
    }
    Ok(zip.finish().map_err(err)?.into_inner())
}

/// Reads a `.modelist` container (checking the format, its version and every part's hash).
pub fn read_container(bytes: &[u8]) -> Result<OpenedContainer, String> {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| format!("not a .modelist file: {e}"))?;
    let read = |zip: &mut zip::ZipArchive<Cursor<&[u8]>>, name: &str| -> Result<Vec<u8>, String> {
        let mut f = zip.by_name(name).map_err(|_| format!("the file has no {name}"))?;
        let mut out = Vec::new();
        f.read_to_end(&mut out).map_err(|e| e.to_string())?;
        Ok(out)
    };
    let mimetype = {
        let mut first = zip.by_index(0).map_err(|e| e.to_string())?;
        let mut out = String::new();
        if first.name().map(|n| n == "mimetype").unwrap_or(false) {
            first.read_to_string(&mut out).map_err(|e| e.to_string())?;
        }
        out
    };
    if mimetype.trim() != MIMETYPE {
        return Err("not a .modelist file (its first entry does not name the format)".into());
    }
    let manifest: Manifest = serde_json::from_slice(&read(&mut zip, MANIFEST)?)
        .map_err(|e| format!("damaged manifest: {e}"))?;
    if manifest.format != "modelist" {
        return Err(format!("unknown format \"{}\"", manifest.format));
    }
    if manifest.format_version > PROJECT_FORMAT_VERSION {
        return Err(format!(
            "this file was written by a newer Modelist (format {}, this build reads up to {PROJECT_FORMAT_VERSION})",
            manifest.format_version
        ));
    }
    let part = |zip: &mut zip::ZipArchive<Cursor<&[u8]>>, path: &str| -> Result<Vec<u8>, String> {
        let data = read(zip, path)?;
        if let Some(mp) = manifest.parts.iter().find(|p| p.path == path) {
            if mp.sha256 != sha256_hex(&data) {
                return Err(format!("{path} is damaged (its checksum does not match)"));
            }
        }
        Ok(data)
    };
    let doc: ProjectDoc = serde_json::from_slice(&part(&mut zip, PROJECT)?)
        .map_err(|e| format!("damaged project.json: {e}"))?;
    let mut components: Vec<GarmentComponent> = Vec::with_capacity(doc.components.len());
    for r in &doc.components {
        let c: GarmentComponent = serde_json::from_slice(&part(&mut zip, &r.path)?)
            .map_err(|e| format!("damaged {}: {e}", r.path))?;
        components.push(c);
    }
    let thumbnail_png = part(&mut zip, THUMBNAIL).ok();
    let session_json = part(&mut zip, SESSION).ok().and_then(|b| String::from_utf8(b).ok());
    let known: Vec<String> = [MANIFEST, PROJECT, THUMBNAIL, SESSION, "mimetype"]
        .iter()
        .map(|s| s.to_string())
        .chain(doc.components.iter().map(|r| r.path.clone()))
        .collect();
    let mut extra_parts = Vec::new();
    for mp in &manifest.parts {
        if known.contains(&mp.path) {
            continue;
        }
        extra_parts.push(ContainerPart {
            path: mp.path.clone(),
            media_type: mp.media_type.clone(),
            bytes: part(&mut zip, &mp.path)?,
        });
    }
    let assembled = PatternProject {
        format_version: doc.format_version,
        id: doc.id,
        name: doc.name,
        measurements: doc.measurements,
        size_table: doc.size_table,
        grade_mode: doc.grade_mode,
        stack_anchor: doc.stack_anchor,
        components,
        active_component: doc.active_component,
    };
    // Same checks and migrations as a plain JSON project.
    let project = PatternProject::from_json(&serde_json::to_string(&assembled).map_err(|e| e.to_string())?)?;
    Ok(OpenedContainer { project, manifest, session_json, thumbnail_png, extra_parts })
}

/// JSON Schema (draft 2020-12) of every JSON part of a `.modelist` container, plus the
/// assembled project as read by the kernel. Published as `docs/format/modelist-v1.schema.json`.
pub fn schema_json() -> String {
    let mut root = serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "https://modelist.app/schema/modelist-v1.schema.json",
        "title": "Modelist project container, format 1",
        "description": "Parts of a .modelist ZIP container. The first ZIP entry is `mimetype` (stored, content application/vnd.modelist+zip). `manifest.json` follows `Manifest`, `project.json` follows `ProjectDoc`, each `components/<id>/component.json` follows `GarmentComponent`. `PatternProject` is the same project assembled into one JSON document (the debug export and the version-0 .modelist.json files).",
        "$defs": {}
    });
    let defs = root["$defs"].as_object_mut().expect("defs");
    for (name, schema) in [
        ("Manifest", schemars::schema_for!(Manifest)),
        ("ProjectDoc", schemars::schema_for!(ProjectDoc)),
        ("GarmentComponent", schemars::schema_for!(GarmentComponent)),
        ("PatternProject", schemars::schema_for!(PatternProject)),
    ] {
        let mut v = serde_json::to_value(schema).expect("schema");
        // Hoist each schema's own definitions so references resolve from the root.
        if let Some(sub) = v.as_object_mut().and_then(|o| o.remove("$defs")) {
            for (k, d) in sub.as_object().cloned().unwrap_or_default() {
                defs.insert(k, d);
            }
        }
        if let Some(o) = v.as_object_mut() {
            o.remove("$schema");
        }
        defs.insert(name.into(), v);
    }
    serde_json::to_string_pretty(&root).expect("schema json") + "\n"
}

/// Pretty JSON for any serializable part (kept as a tiny trait object helper).
mod erased {
    pub trait Json {
        fn pretty(&self) -> serde_json::Result<String>;
    }
    impl<T: serde::Serialize> Json for T {
        fn pretty(&self) -> serde_json::Result<String> {
            serde_json::to_string_pretty(self)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Point2;

    fn two_garments() -> PatternProject {
        let mut p = crate::library::new_project("boxy_tee");
        p.components.push(crate::library::new_component("wide_pleat_trouser", 1, Point2::new(1500.0, 0.0)));
        p.components[1].layer = 1;
        p.active_component = 1;
        p
    }

    #[test]
    fn a_project_round_trips_with_its_session_thumbnail_and_unknown_parts() {
        let p = two_garments();
        let extra = ContainerPart {
            path: "trims/button.4hole.15mm.json".into(),
            media_type: "application/json".into(),
            bytes: br#"{"id":"button.4hole.15mm"}"#.to_vec(),
        };
        let png = [0x89u8, b'P', b'N', b'G', 1, 2, 3];
        let bytes = write_container(
            &p,
            &WriteOptions {
                writer_version: "0.2.0",
                modified: Some("2026-11-24T12:00:00Z"),
                session_json: Some(r#"{"camera":"front"}"#),
                thumbnail_png: Some(&png),
                extra_parts: std::slice::from_ref(&extra),
            },
        )
        .unwrap();
        assert!(is_container(&bytes));
        // The first entry names the format, uncompressed, right after its 30-byte local header.
        assert_eq!(&bytes[30..38], b"mimetype");
        assert_eq!(&bytes[38..38 + MIMETYPE.len()], MIMETYPE.as_bytes());

        let opened = read_container(&bytes).unwrap();
        assert_eq!(opened.project, p);
        assert_eq!(opened.session_json.as_deref(), Some(r#"{"camera":"front"}"#));
        assert_eq!(opened.thumbnail_png.as_deref(), Some(&png[..]));
        assert_eq!(opened.extra_parts, vec![extra]);
        assert_eq!(opened.manifest.writer.version, "0.2.0");
        assert_eq!(opened.manifest.parts.len(), 6, "project, 2 garments, thumbnail, session, trims");
        // One part per garment, so a garment's history diffs on its own.
        assert!(opened.manifest.parts.iter().any(|m| m.path == format!("components/{}/component.json", p.components[1].id)));
    }

    #[test]
    fn damaged_or_newer_files_are_refused() {
        let p = two_garments();
        let bytes = write_container(&p, &WriteOptions { writer_version: "0.2.0", ..Default::default() }).unwrap();
        // Copy the container with project.json edited but the manifest unchanged.
        let mut src = zip::ZipArchive::new(Cursor::new(&bytes[..])).unwrap();
        let mut out = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for i in 0..src.len() {
            let mut f = src.by_index(i).unwrap();
            let name = f.name().unwrap().to_string();
            let mut data = Vec::new();
            f.read_to_end(&mut data).unwrap();
            if name == PROJECT {
                data = String::from_utf8(data).unwrap().replace(&p.name, "Tampered").into_bytes();
            }
            out.start_file(name.as_str(), SimpleFileOptions::default().compression_method(CompressionMethod::Stored)).unwrap();
            out.write_all(&data).unwrap();
        }
        let tampered = out.finish().unwrap().into_inner();
        let e = read_container(&tampered).unwrap_err();
        assert!(e.contains("damaged"), "{e}");

        assert!(read_container(b"PK\x03\x04 not really a zip").is_err());
        assert!(read_container(br#"{"format":"modelist"}"#).is_err());
    }

    /// The published schema must match the code. Regenerate with
    /// `MODELIST_UPDATE_SCHEMA=1 cargo test -p modelist-core published_schema`.
    #[test]
    fn published_schema_is_up_to_date() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/format/modelist-v1.schema.json");
        let fresh = schema_json();
        if std::env::var("MODELIST_UPDATE_SCHEMA").is_ok() {
            std::fs::create_dir_all(std::path::Path::new(path).parent().unwrap()).unwrap();
            std::fs::write(path, &fresh).unwrap();
        }
        let current = std::fs::read_to_string(path).unwrap_or_default();
        assert!(current == fresh, "docs/format/modelist-v1.schema.json is stale: run with MODELIST_UPDATE_SCHEMA=1");
        // Every part's schema is there.
        let v: serde_json::Value = serde_json::from_str(&fresh).unwrap();
        for name in ["Manifest", "ProjectDoc", "GarmentComponent", "PatternProject", "ParametricOp", "FabricSpec"] {
            assert!(v["$defs"].get(name).is_some(), "missing {name}");
        }
    }
}
