use super::model::{ArtifactDescriptor, EvidenceSemantics, ParsedArtifact};
use super::reader::{scalar_record, validate_json_depth, ReadArtifact};
use anyhow::{bail, Result};

pub fn parse(
    read: ReadArtifact,
    descriptor: &ArtifactDescriptor,
    redacted: &mut u64,
) -> Result<ParsedArtifact> {
    let value: serde_json::Value = serde_json::from_slice(&read.bytes)?;
    validate_json_depth(&value, 0)?;
    let values = if let Some(array) = value.as_array() {
        array.clone()
    } else if let Some(array) = value.get("records").and_then(serde_json::Value::as_array) {
        array.clone()
    } else if value.is_object() {
        vec![value]
    } else {
        bail!("BYOND evidence must be an object or array");
    };
    if values.len() > crate::limits::MAX_EVIDENCE_ROWS {
        bail!("evidence row limit exceeded");
    }
    let records = values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            scalar_record(value, index as u64, descriptor.options.as_ref(), redacted)
        })
        .collect::<Vec<_>>();
    Ok(ParsedArtifact {
        identity: read.identity,
        semantics: EvidenceSemantics::CumulativeSnapshot,
        accepted_records: records.len(),
        rejected_records: 0,
        records,
        unavailable_metrics: Vec::new(),
    })
}
