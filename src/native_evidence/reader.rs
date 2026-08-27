use crate::limits::{MAX_EVIDENCE_FILE_BYTES, MAX_EVIDENCE_TOTAL_BYTES};
use crate::native_evidence::model::{ArtifactDescriptor, ArtifactIdentity};
use crate::PathPolicy;
use anyhow::{bail, Result};
use sha2::{Digest, Sha256};

pub struct ReadArtifact {
    pub identity: ArtifactIdentity,
    pub bytes: Vec<u8>,
}

pub fn read_artifact(
    policy: &PathPolicy,
    descriptor: &ArtifactDescriptor,
    total: &mut u64,
) -> Result<ReadArtifact> {
    let path = policy.read_path(&descriptor.path)?;
    let metadata = std::fs::metadata(&path)?;
    if metadata.len() > MAX_EVIDENCE_FILE_BYTES {
        bail!("evidence file exceeds fixed byte limit");
    }
    *total = total
        .checked_add(metadata.len())
        .ok_or_else(|| anyhow::anyhow!("evidence byte total overflow"))?;
    if *total > MAX_EVIDENCE_TOTAL_BYTES {
        bail!("evidence request exceeds fixed total byte limit");
    }
    let bytes = std::fs::read(&path)?;
    if bytes.len() as u64 > MAX_EVIDENCE_FILE_BYTES {
        bail!("evidence file grew beyond fixed byte limit");
    }
    let root = policy
        .effective_roots()
        .iter()
        .filter(|root| path.starts_with(&root.path))
        .max_by_key(|root| root.path.components().count())
        .ok_or_else(|| anyhow::anyhow!("evidence path has no effective root"))?;
    let relative_path = path
        .strip_prefix(&root.path)?
        .to_string_lossy()
        .replace('\\', "/");
    Ok(ReadArtifact {
        identity: ArtifactIdentity {
            relative_path,
            kind: descriptor.kind,
            bytes: bytes.len() as u64,
            sha256: format!("{:x}", Sha256::digest(&bytes)),
        },
        bytes,
    })
}

pub fn validate_json_depth(value: &serde_json::Value, depth: usize) -> Result<()> {
    if depth > 64 {
        bail!("evidence JSON exceeds fixed depth limit");
    }
    match value {
        serde_json::Value::Array(values) => {
            for value in values {
                validate_json_depth(value, depth + 1)?;
            }
        }
        serde_json::Value::Object(values) => {
            for value in values.values() {
                validate_json_depth(value, depth + 1)?;
            }
        }
        serde_json::Value::String(value)
            if value.len() > crate::limits::MAX_EVIDENCE_STRING_BYTES =>
        {
            bail!("evidence JSON string exceeds fixed byte limit");
        }
        _ => {}
    }
    Ok(())
}

pub fn scalar_record(
    value: &serde_json::Value,
    index: u64,
    options: Option<&crate::native_evidence::model::ArtifactOptions>,
    redacted: &mut u64,
) -> crate::native_evidence::model::EvidenceRecord {
    let mut metrics = std::collections::BTreeMap::new();
    let mut groups = std::collections::BTreeMap::new();
    let mut wall_unix_ms = None;
    let mut world_deciseconds = None;
    if let Some(object) = value.as_object() {
        for (name, value) in object {
            if options.and_then(|item| item.wall_time_field.as_deref()) == Some(name) {
                wall_unix_ms = parse_wall(value);
                continue;
            }
            if options.and_then(|item| item.world_time_field.as_deref()) == Some(name) {
                world_deciseconds = value.as_i64();
                continue;
            }
            if let Some(number) = value.as_f64() {
                let selected = options.is_none_or(|item| {
                    item.selected_metrics.is_empty()
                        || item.selected_metrics.iter().any(|metric| metric == name)
                });
                if selected && number.is_finite() {
                    metrics.insert(name.clone(), number);
                }
            } else if let Some(text) = value.as_str() {
                if crate::native_evidence::redaction::protected(name) {
                    *redacted += 1;
                    continue;
                }
                if options.is_some_and(|item| item.group_fields.iter().any(|field| field == name)) {
                    let (text, count) = crate::native_evidence::redaction::sanitize_text(text);
                    *redacted += count;
                    if text.len() <= crate::limits::MAX_EVIDENCE_STRING_BYTES {
                        groups.insert(name.clone(), text);
                    }
                }
            }
        }
    }
    crate::native_evidence::model::EvidenceRecord {
        wall_unix_ms,
        world_deciseconds,
        sample_index: index,
        metrics,
        groups,
    }
}

fn parse_wall(value: &serde_json::Value) -> Option<i128> {
    if let Some(number) = value.as_i64() {
        return Some(number as i128);
    }
    let text = value.as_str()?;
    time::OffsetDateTime::parse(text, &time::format_description::well_known::Rfc3339)
        .ok()
        .map(|time| time.unix_timestamp_nanos() / 1_000_000)
}
