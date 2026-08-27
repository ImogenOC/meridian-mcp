use crate::capabilities::SPACEMANDMM_REVISION;
use crate::index::LanguageIndex;
use crate::proc_resolution::ProcResolver;
use crate::search::SearchIndex;
use crate::spaceman::dmi::{IconReference, IconReferenceResolution};
use crate::spaceman::language::ReferenceTable;
use crate::ProjectProfile;
use dreammaker::config::Config;
use dreammaker::constants::{ConstFn, Constant};
use dreammaker::objtree::ObjectTree;
use dreammaker::preprocessor::DefineHistory;
use dreammaker::{Context, FileId, Location};
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub struct DiagnosticRecord {
    pub rule: Option<String>,
    pub severity: String,
    pub component: String,
    pub message: String,
    pub file: String,
    pub line: u32,
    pub column: u16,
    pub notes: Vec<DiagnosticNoteRecord>,
    pub configured: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub struct DiagnosticNoteRecord {
    pub message: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub struct MacroDefinitionRecord {
    pub name: String,
    pub file: String,
    pub line: u32,
    pub column: u16,
    pub parameters: Vec<String>,
    pub variadic: bool,
    pub definition: String,
}

#[derive(Clone, Debug)]
pub struct AnalysisContext {
    pub config: Config,
    file_paths: HashMap<FileId, PathBuf>,
}

impl AnalysisContext {
    pub fn file_path(&self, file: FileId) -> &Path {
        self.file_paths
            .get(&file)
            .map(PathBuf::as_path)
            .unwrap_or_else(|| Path::new("(unknown)"))
    }

    fn extract(
        context: &Context,
        objtree: &ObjectTree,
        defines: &DefineHistory,
        environment_path: &Path,
    ) -> Self {
        let mut file_paths = HashMap::new();
        let project_root = environment_path.parent().unwrap_or_else(|| Path::new("."));
        let mut capture = |location: Location| {
            if location.file != FileId::INVALID {
                let reported = context.file_path(location.file);
                let resolved = if reported.is_absolute() {
                    reported.to_path_buf()
                } else {
                    project_root.join(&*reported)
                };
                file_paths.entry(location.file).or_insert(resolved);
            }
        };

        for ty in objtree.iter_types() {
            capture(ty.location);
            for (_, var) in &ty.vars {
                capture(var.value.location);
            }
            for (_, proc) in &ty.procs {
                for value in &proc.value {
                    capture(value.location);
                }
            }
        }
        for (range, _) in defines.iter() {
            capture(range.start);
        }
        for error in context.errors().iter() {
            capture(error.location());
        }

        Self {
            config: context.config().clone(),
            file_paths,
        }
    }
}

pub struct AnalysisBuild {
    pub environment_path: PathBuf,
    pub context: AnalysisContext,
    pub objtree: ObjectTree,
    pub macro_definitions: Vec<MacroDefinitionRecord>,
    pub(crate) search_index: SearchIndex,
    pub diagnostics: Vec<DiagnosticRecord>,
    pub project_profile: Option<ProjectProfile>,
    pub language_index: LanguageIndex,
    pub reference_table: ReferenceTable,
    pub icon_references: Vec<IconReference>,
    pub proc_resolver: ProcResolver,
    pub source_inputs: Vec<PathBuf>,
}

impl AnalysisBuild {
    pub(crate) fn from_parse(
        environment_path: PathBuf,
        context: &Context,
        objtree: ObjectTree,
        defines: DefineHistory,
        search_index: SearchIndex,
        diagnostics: Vec<DiagnosticRecord>,
        project_profile: Option<ProjectProfile>,
    ) -> Self {
        let extracted_context =
            AnalysisContext::extract(context, &objtree, &defines, &environment_path);
        let project_root = environment_path.parent().unwrap_or_else(|| Path::new("."));
        let macro_definitions: Vec<MacroDefinitionRecord> = defines
            .iter()
            .map(|(range, (name, define))| MacroDefinitionRecord {
                name: name.to_string(),
                file: {
                    let reported = context.file_path(range.start.file);
                    if reported.is_absolute() {
                        reported.to_path_buf()
                    } else {
                        project_root.join(&*reported)
                    }
                    .display()
                    .to_string()
                },
                line: range.start.line,
                column: range.start.column,
                parameters: define.params.iter().map(ToString::to_string).collect(),
                variadic: define.variadic,
                definition: define.display_with_name(name).to_string(),
            })
            .collect();
        let proc_resolver = ProcResolver::build(&extracted_context, &objtree);
        let search_index = search_index.with_proc_resolver(&proc_resolver);
        let source_inputs = build_source_inputs(
            &extracted_context,
            &environment_path,
            project_profile.as_ref(),
        );
        let language_index = LanguageIndex::build(
            &extracted_context,
            &objtree,
            &macro_definitions,
            &proc_resolver,
        );
        let reference_table = ReferenceTable::build(&objtree);
        let icon_references =
            build_icon_references(&extracted_context, &objtree, &environment_path);
        Self {
            environment_path,
            context: extracted_context,
            objtree,
            macro_definitions,
            search_index,
            diagnostics,
            project_profile,
            language_index,
            reference_table,
            icon_references,
            proc_resolver,
            source_inputs,
        }
    }
}

pub struct AnalysisSnapshot {
    pub environment_path: PathBuf,
    pub context: Arc<AnalysisContext>,
    pub objtree: Arc<ObjectTree>,
    pub macro_definitions: Arc<[MacroDefinitionRecord]>,
    pub(crate) search_index: Arc<SearchIndex>,
    pub diagnostics: Arc<[DiagnosticRecord]>,
    pub project_profile: Option<ProjectProfile>,
    pub language_index: Arc<LanguageIndex>,
    pub reference_table: Arc<ReferenceTable>,
    pub icon_references: Arc<[IconReference]>,
    pub proc_resolver: Arc<ProcResolver>,
    pub source_inputs: Arc<[PathBuf]>,
    pub generation: u64,
    pub spacemandmm_revision: &'static str,
}

impl AnalysisSnapshot {
    pub(crate) fn from_build(build: AnalysisBuild, generation: u64) -> Self {
        Self {
            environment_path: build.environment_path,
            context: Arc::new(build.context),
            objtree: Arc::new(build.objtree),
            macro_definitions: Arc::from(build.macro_definitions),
            search_index: Arc::new(build.search_index),
            diagnostics: Arc::from(build.diagnostics),
            project_profile: build.project_profile,
            language_index: Arc::new(build.language_index),
            reference_table: Arc::new(build.reference_table),
            icon_references: Arc::from(build.icon_references),
            proc_resolver: Arc::new(build.proc_resolver),
            source_inputs: Arc::from(build.source_inputs),
            generation,
            spacemandmm_revision: SPACEMANDMM_REVISION,
        }
    }

    pub fn proc_resolver(&self) -> &ProcResolver {
        &self.proc_resolver
    }

    pub fn source_inputs(&self) -> &[PathBuf] {
        &self.source_inputs
    }
}

fn build_source_inputs(
    context: &AnalysisContext,
    environment_path: &Path,
    profile: Option<&ProjectProfile>,
) -> Vec<PathBuf> {
    let project_root = environment_path.parent().unwrap_or_else(|| Path::new("."));
    let project_root = project_root
        .canonicalize()
        .unwrap_or_else(|_| project_root.to_owned());
    let mut inputs = context.file_paths.values().cloned().collect::<Vec<_>>();
    inputs.push(environment_path.to_owned());
    if let Some(config) = profile.and_then(ProjectProfile::spaceman_config) {
        inputs.push(config.to_owned());
    }
    inputs = inputs
        .into_iter()
        .filter_map(|path| path.canonicalize().ok())
        .filter(|path| path.is_file() && path.starts_with(&project_root))
        .collect();
    inputs.sort();
    inputs.dedup();
    inputs
}

fn build_icon_references(
    context: &AnalysisContext,
    objtree: &ObjectTree,
    environment_path: &Path,
) -> Vec<IconReference> {
    let project_root = environment_path.parent().unwrap_or_else(|| Path::new("."));
    let mut references = Vec::new();
    for ty in objtree.iter_types() {
        let direct_icon = ty.vars.get("icon");
        let direct_state = ty.vars.get("icon_state");
        if direct_icon.is_none() && direct_state.is_none() {
            continue;
        }
        let location = direct_state
            .map(|value| value.value.location)
            .or_else(|| direct_icon.map(|value| value.value.location))
            .unwrap_or(ty.location);
        let file = context.file_path(location.file).display().to_string();
        let icon_value = ty.get_value("icon");
        let state_value = ty.get_value("icon_state");
        let resolution = match icon_value.and_then(|value| value.constant.as_ref()) {
            Some(Constant::Null(_)) | None
                if icon_value.is_none_or(|value| value.expression.is_none()) =>
            {
                continue;
            }
            Some(Constant::Resource(resource)) => {
                static_icon_resolution(project_root, resource, state_value)
            }
            Some(Constant::Call(ConstFn::Icon, arguments)) => {
                match arguments.first().map(|(value, _)| value) {
                    Some(Constant::Resource(resource)) => {
                        static_icon_resolution(project_root, resource, state_value)
                    }
                    _ => IconReferenceResolution::Dynamic {
                        reason: "icon() resource is not statically resolvable".to_owned(),
                    },
                }
            }
            _ => IconReferenceResolution::Dynamic {
                reason: "icon expression is not a literal resource".to_owned(),
            },
        };
        references.push(IconReference {
            type_path: ty.path.to_string(),
            file,
            line: location.line,
            resolution,
        });
    }
    references.sort_by(|left, right| {
        (&left.file, left.line, &left.type_path).cmp(&(&right.file, right.line, &right.type_path))
    });
    references
}

fn static_icon_resolution(
    project_root: &Path,
    resource: &str,
    state_value: Option<&dreammaker::objtree::VarValue>,
) -> IconReferenceResolution {
    let dmi_path = project_root.join(resource.replace('\\', "/"));
    let state = match state_value.and_then(|value| value.constant.as_ref()) {
        Some(Constant::String(state)) => Some(state.to_string()),
        Some(Constant::Null(_)) | None
            if state_value.is_none_or(|value| value.expression.is_none()) =>
        {
            None
        }
        _ => {
            return IconReferenceResolution::Dynamic {
                reason: format!(
                    "icon_state expression is dynamic for statically resolved {}",
                    dmi_path.display()
                ),
            };
        }
    };
    IconReferenceResolution::Static { dmi_path, state }
}

pub(crate) fn configured_diagnostic_rules(environment_path: &Path) -> HashSet<String> {
    let Some(root) = environment_path.parent() else {
        return HashSet::new();
    };
    let Ok(source) = std::fs::read_to_string(root.join("SpacemanDMM.toml")) else {
        return HashSet::new();
    };
    let Ok(value) = source.parse::<toml::Value>() else {
        return HashSet::new();
    };
    value
        .get("diagnostics")
        .and_then(toml::Value::as_table)
        .map(|table| table.keys().cloned().collect())
        .unwrap_or_default()
}

pub(crate) fn collect_diagnostics(
    context: &Context,
    configured_rules: &HashSet<String>,
) -> Vec<DiagnosticRecord> {
    context
        .errors()
        .iter()
        .map(|error| {
            let location = error.location();
            DiagnosticRecord {
                rule: error.errortype().map(str::to_owned),
                severity: error.severity().to_string(),
                component: error.component().name().unwrap_or("parser").to_owned(),
                message: error.description().to_owned(),
                file: context.file_path(location.file).display().to_string(),
                line: location.line,
                column: location.column,
                notes: error
                    .notes()
                    .iter()
                    .map(|note| DiagnosticNoteRecord {
                        message: format!("{note:?}"),
                    })
                    .collect(),
                configured: error
                    .errortype()
                    .is_some_and(|rule| configured_rules.contains(rule)),
            }
        })
        .collect()
}
