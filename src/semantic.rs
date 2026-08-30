use anyhow::{anyhow, Result};
use sha2::{Digest, Sha256};
use std::path::{Component, Path, PathBuf};

pub const SEMANTIC_CHUNK_SCHEMA_VERSION: u32 = 1;
const CHUNK_LINES: usize = 40;
const CHUNK_OVERLAP_LINES: usize = 5;

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub struct SemanticChunkRecord {
    pub schema_version: u32,
    pub chunk_id: String,
    pub document_id: String,
    pub content_digest: String,
    pub chunk_index: u32,
    pub kind: String,
    pub symbol: String,
    pub implementation_owner: Option<String>,
    pub declaration_owner: Option<String>,
    pub repository_relative_file: String,
    pub line: u32,
    pub column: u32,
    pub text: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub struct VectorIndexIdentity {
    pub chunk_schema_version: u32,
    pub embedding_provider: String,
    pub embedding_model: String,
    pub dimensions: usize,
    pub distance: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticDocumentInput {
    pub kind: String,
    pub symbol: String,
    pub implementation_owner: Option<String>,
    pub declaration_owner: Option<String>,
    pub file: PathBuf,
    pub line: u32,
    pub column: u32,
    pub override_index: Option<usize>,
    pub docs: String,
    pub source: Option<String>,
}

pub fn build_semantic_chunks(
    document: &SemanticDocumentInput,
    repository_root: &Path,
) -> Result<Vec<SemanticChunkRecord>> {
    let relative_file = repository_relative_path(&document.file, repository_root)?;
    let kind = document.kind.clone();
    let override_index = document
        .override_index
        .map(|index| index.to_string())
        .unwrap_or_else(|| "none".to_owned());
    let document_id = digest_parts(&[
        "semantic-document",
        &SEMANTIC_CHUNK_SCHEMA_VERSION.to_string(),
        &kind,
        &document.symbol,
        document.implementation_owner.as_deref().unwrap_or(""),
        document.declaration_owner.as_deref().unwrap_or(""),
        &relative_file,
        &document.line.to_string(),
        &document.column.to_string(),
        &override_index,
    ]);
    let text = semantic_text(document);
    let lines = text.lines().collect::<Vec<_>>();
    let mut chunks = Vec::new();
    let mut start = 0_usize;
    while start < lines.len() {
        let end = lines.len().min(start + CHUNK_LINES);
        let chunk_text = lines[start..end].join("\n");
        let chunk_index = u32::try_from(chunks.len())
            .map_err(|_| anyhow!("semantic document has too many chunks"))?;
        chunks.push(SemanticChunkRecord {
            schema_version: SEMANTIC_CHUNK_SCHEMA_VERSION,
            chunk_id: digest_parts(&["semantic-chunk", &document_id, &chunk_index.to_string()]),
            document_id: document_id.clone(),
            content_digest: digest_parts(&["semantic-content", &chunk_text]),
            chunk_index,
            kind: kind.clone(),
            symbol: document.symbol.clone(),
            implementation_owner: document.implementation_owner.clone(),
            declaration_owner: document.declaration_owner.clone(),
            repository_relative_file: relative_file.clone(),
            line: document.line,
            column: document.column,
            text: chunk_text,
        });
        if end == lines.len() {
            break;
        }
        start = end - CHUNK_OVERLAP_LINES;
    }
    Ok(chunks)
}

fn semantic_text(document: &SemanticDocumentInput) -> String {
    [
        Some(document.symbol.as_str()),
        (!document.docs.is_empty()).then_some(document.docs.as_str()),
        document
            .source
            .as_deref()
            .filter(|source| !source.is_empty()),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join("\n")
}

fn repository_relative_path(file: &Path, repository_root: &Path) -> Result<String> {
    let relative = file.strip_prefix(repository_root).map_err(|_| {
        anyhow!(
            "semantic source {} is outside repository root {}",
            file.display(),
            repository_root.display()
        )
    })?;
    if relative.as_os_str().is_empty()
        || relative.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(anyhow!("semantic source path must be repository-relative"));
    }
    Ok(relative.to_string_lossy().replace('\\', "/"))
}

fn digest_parts(parts: &[&str]) -> String {
    let mut digest = Sha256::new();
    for part in parts {
        digest.update((part.len() as u64).to_be_bytes());
        digest.update(part.as_bytes());
    }
    format!("{:x}", digest.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn document(source: &str) -> SemanticDocumentInput {
        SemanticDocumentInput {
            kind: "proc".to_owned(),
            symbol: "/datum/example/proc/run".to_owned(),
            implementation_owner: Some("/datum/example".to_owned()),
            declaration_owner: Some("/datum/example".to_owned()),
            file: PathBuf::from("C:/repository/code/example.dm"),
            line: 12,
            column: 1,
            docs: "Run the example.".to_owned(),
            source: Some(source.to_owned()),
            override_index: Some(1),
        }
    }

    #[test]
    fn semantic_chunk_identity_is_stable_and_content_sensitive() {
        let root = Path::new("C:/repository");
        let first = build_semantic_chunks(&document("return 1"), root).unwrap();
        let rebuilt = build_semantic_chunks(&document("return 1"), root).unwrap();
        let edited = build_semantic_chunks(&document("return 2"), root).unwrap();

        assert_eq!(first, rebuilt);
        assert_eq!(first[0].document_id, edited[0].document_id);
        assert_ne!(first[0].content_digest, edited[0].content_digest);
        assert_eq!(first[0].repository_relative_file, "code/example.dm");
        assert!(!first[0].repository_relative_file.contains("repository"));
    }

    #[test]
    fn vector_model_identity_does_not_change_chunks() {
        let root = Path::new("C:/repository");
        let before = build_semantic_chunks(&document("return 1"), root).unwrap();
        let _first_model = VectorIndexIdentity {
            chunk_schema_version: SEMANTIC_CHUNK_SCHEMA_VERSION,
            embedding_provider: "provider-a".to_owned(),
            embedding_model: "model-a".to_owned(),
            dimensions: 1_024,
            distance: "cosine".to_owned(),
        };
        let _second_model = VectorIndexIdentity {
            chunk_schema_version: SEMANTIC_CHUNK_SCHEMA_VERSION,
            embedding_provider: "provider-b".to_owned(),
            embedding_model: "model-b".to_owned(),
            dimensions: 3_072,
            distance: "cosine".to_owned(),
        };
        let after = build_semantic_chunks(&document("return 1"), root).unwrap();

        assert_eq!(before, after);
    }

    #[test]
    fn long_documents_use_forty_line_chunks_with_five_line_overlap() {
        let source = (0..85)
            .map(|line| format!("line {line}"))
            .collect::<Vec<_>>()
            .join("\n");
        let chunks = build_semantic_chunks(&document(&source), Path::new("C:/repository")).unwrap();

        assert_eq!(chunks.len(), 3);
        let first = chunks[0].text.lines().collect::<Vec<_>>();
        let second = chunks[1].text.lines().collect::<Vec<_>>();
        assert_eq!(&first[first.len() - 5..], &second[..5]);
        assert!(chunks.iter().all(|chunk| {
            chunk.chunk_id.len() == 64
                && chunk.content_digest.len() == 64
                && chunk
                    .chunk_id
                    .bytes()
                    .chain(chunk.content_digest.bytes())
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        }));
    }
}
