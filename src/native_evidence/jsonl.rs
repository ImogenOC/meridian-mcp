use super::model::{ArtifactDescriptor, ArtifactKind, EvidenceSemantics, ParsedArtifact};
use super::reader::{scalar_record, validate_json_depth, ReadArtifact};
use anyhow::{bail, Result};

pub fn parse(
    read: ReadArtifact,
    descriptor: &ArtifactDescriptor,
    redacted: &mut u64,
) -> Result<ParsedArtifact> {
    let text = std::str::from_utf8(&read.bytes)?;
    let mut records = Vec::new();
    for (index, line) in text.lines().enumerate() {
        if line.len() > crate::limits::MAX_EVIDENCE_LINE_BYTES {
            bail!("evidence line limit exceeded");
        }
        if line.trim().is_empty() {
            continue;
        }
        if records.len() >= crate::limits::MAX_EVIDENCE_ROWS {
            bail!("evidence row limit exceeded");
        }
        let value: serde_json::Value = serde_json::from_str(line)?;
        validate_json_depth(&value, 0)?;
        records.push(scalar_record(
            &value,
            index as u64,
            descriptor.options.as_ref(),
            redacted,
        ));
    }
    let semantics = match descriptor.kind {
        ArtifactKind::RuntimeJsonl | ArtifactKind::EventJsonl => EvidenceSemantics::EventStream,
        _ => unreachable!(),
    };
    Ok(ParsedArtifact {
        identity: read.identity,
        semantics,
        accepted_records: records.len(),
        rejected_records: 0,
        records,
        unavailable_metrics: Vec::new(),
    })
}
