use crate::analysis_snapshot::{AnalysisContext, MacroDefinitionRecord};
use dreammaker::objtree::ObjectTree;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SymbolId {
    Type {
        path: String,
    },
    Proc {
        owner: String,
        name: String,
        override_index: usize,
    },
    Var {
        owner: String,
        name: String,
    },
    Macro {
        name: String,
        file: String,
        line: u32,
    },
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SymbolKind {
    Type,
    Proc,
    Var,
    Macro,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DocumentSymbol {
    pub id: SymbolId,
    pub name: String,
    pub kind: SymbolKind,
    pub owner: Option<String>,
    pub file: String,
    pub line: u32,
    pub column: u16,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceKind {
    Call,
    Read,
    Write,
    TypePath,
    MacroExpansion,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ReferenceHit {
    pub symbol: SymbolId,
    pub kind: ReferenceKind,
    pub file: String,
    pub line: u32,
    pub column: u16,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ImplementationHit {
    pub symbol: SymbolId,
    pub declared_in: String,
    pub inherited_from: Option<String>,
    pub file: String,
    pub line: u32,
    pub column: u16,
}

#[derive(Clone, Debug, Default)]
pub struct LanguageIndex {
    documents: BTreeMap<PathBuf, Vec<DocumentSymbol>>,
    implementations: Vec<ImplementationHit>,
}

impl LanguageIndex {
    pub fn build(
        context: &AnalysisContext,
        objtree: &ObjectTree,
        macros: &[MacroDefinitionRecord],
    ) -> Self {
        let mut index = Self::default();
        for macro_record in macros {
            index.insert(DocumentSymbol {
                id: SymbolId::Macro {
                    name: macro_record.name.clone(),
                    file: macro_record.file.clone(),
                    line: macro_record.line,
                },
                name: macro_record.name.clone(),
                kind: SymbolKind::Macro,
                owner: None,
                file: macro_record.file.clone(),
                line: macro_record.line,
                column: macro_record.column,
            });
        }
        for ty in objtree.iter_types() {
            let owner = ty.path.to_string();
            let file = context.file_path(ty.location.file).display().to_string();
            index.insert(DocumentSymbol {
                id: SymbolId::Type {
                    path: owner.clone(),
                },
                name: owner.rsplit('/').next().unwrap_or("/").to_owned(),
                kind: SymbolKind::Type,
                owner: None,
                file: file.clone(),
                line: ty.location.line,
                column: ty.location.column,
            });
            index.implementations.push(ImplementationHit {
                symbol: SymbolId::Type {
                    path: owner.clone(),
                },
                declared_in: owner.clone(),
                inherited_from: ty.parent_type().map(|parent| parent.path.to_string()),
                file: file.clone(),
                line: ty.location.line,
                column: ty.location.column,
            });
            for (name, var) in &ty.vars {
                if var.declaration.is_none() {
                    continue;
                }
                let file = context
                    .file_path(var.value.location.file)
                    .display()
                    .to_string();
                let symbol = SymbolId::Var {
                    owner: owner.clone(),
                    name: name.to_string(),
                };
                index.insert(DocumentSymbol {
                    id: symbol.clone(),
                    name: name.to_string(),
                    kind: SymbolKind::Var,
                    owner: Some(owner.clone()),
                    file: file.clone(),
                    line: var.value.location.line,
                    column: var.value.location.column,
                });
                index.implementations.push(ImplementationHit {
                    symbol,
                    declared_in: owner.clone(),
                    inherited_from: None,
                    file,
                    line: var.value.location.line,
                    column: var.value.location.column,
                });
            }
            for proc_ref in ty.iter_self_procs() {
                let value = proc_ref.get();
                let file = context.file_path(value.location.file).display().to_string();
                let symbol = SymbolId::Proc {
                    owner: owner.clone(),
                    name: proc_ref.name().to_owned(),
                    override_index: proc_ref.index(),
                };
                index.insert(DocumentSymbol {
                    id: symbol.clone(),
                    name: proc_ref.name().to_owned(),
                    kind: SymbolKind::Proc,
                    owner: Some(owner.clone()),
                    file: file.clone(),
                    line: value.location.line,
                    column: value.location.column,
                });
                index.implementations.push(ImplementationHit {
                    symbol,
                    declared_in: owner.clone(),
                    inherited_from: proc_ref
                        .parent_proc()
                        .map(|parent| parent.ty().path.to_string()),
                    file,
                    line: value.location.line,
                    column: value.location.column,
                });
            }
        }
        for symbols in index.documents.values_mut() {
            symbols.sort_by(|left, right| {
                (left.line, left.column, left.kind, &left.name).cmp(&(
                    right.line,
                    right.column,
                    right.kind,
                    &right.name,
                ))
            });
            symbols.dedup();
        }
        index.implementations.sort_by(|left, right| {
            (&left.declared_in, left.line, left.column).cmp(&(
                &right.declared_in,
                right.line,
                right.column,
            ))
        });
        index
    }

    fn insert(&mut self, symbol: DocumentSymbol) {
        self.documents
            .entry(normalize_path(Path::new(&symbol.file)))
            .or_default()
            .push(symbol);
    }

    pub fn document_symbols(&self, file: &Path) -> &[DocumentSymbol] {
        self.documents
            .get(&normalize_path(file))
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    pub fn macros(&self) -> impl Iterator<Item = &DocumentSymbol> {
        self.documents
            .values()
            .flatten()
            .filter(|symbol| symbol.kind == SymbolKind::Macro)
    }

    pub fn source_files(&self) -> Vec<PathBuf> {
        let mut files = self
            .documents
            .values()
            .flatten()
            .map(|symbol| PathBuf::from(&symbol.file))
            .collect::<Vec<_>>();
        files.sort();
        files.dedup();
        files
    }

    pub fn proc_at(&self, file: &Path, line: u32) -> Option<&DocumentSymbol> {
        self.document_symbols(file)
            .iter()
            .take_while(|symbol| symbol.line <= line)
            .last()
            .filter(|symbol| symbol.kind == SymbolKind::Proc)
    }

    pub fn implementations(&self, owner: &str, member: Option<&str>) -> Vec<ImplementationHit> {
        self.implementations
            .iter()
            .filter(|hit| match (&hit.symbol, member) {
                (SymbolId::Type { path }, None) => {
                    path == owner || path.starts_with(&format!("{owner}/"))
                }
                (SymbolId::Proc { name, .. } | SymbolId::Var { name, .. }, Some(member)) => {
                    name == member
                        && (hit.declared_in == owner
                            || hit.declared_in.starts_with(&format!("{owner}/")))
                }
                _ => false,
            })
            .cloned()
            .collect()
    }
}

fn normalize_path(path: &Path) -> PathBuf {
    let text = path
        .to_string_lossy()
        .replace('\\', "/")
        .to_ascii_lowercase();
    PathBuf::from(text)
}
