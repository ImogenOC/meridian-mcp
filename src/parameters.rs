use schemars::JsonSchema;
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ParseEnvironmentParams {
    pub dme_path: PathBuf,
}
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetTypeParams {
    pub type_path: String,
}
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetProcParams {
    pub type_path: String,
    pub proc_name: String,
}
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetVarParams {
    pub type_path: String,
    pub var_name: String,
}
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListTypesParams {
    pub prefix: Option<String>,
    pub max_depth: Option<usize>,
}
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchSymbolsParams {
    pub query: String,
    pub kind: Option<String>,
    pub limit: Option<usize>,
}
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchContextParams {
    pub query: String,
    pub kind: Option<String>,
    pub type_prefix: Option<String>,
    pub file_filter: Option<String>,
    pub limit: Option<usize>,
    pub include_source: Option<bool>,
    pub max_source_lines: Option<usize>,
}
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CheckErrorsParams {
    pub file_path: Option<String>,
}
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetDefinitionParams {
    pub type_path: String,
    pub member_name: Option<String>,
}
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CompileParams {
    pub dme_path: PathBuf,
    pub compiler_path: Option<PathBuf>,
    pub working_directory: Option<PathBuf>,
    #[serde(default)]
    pub defines: Vec<String>,
    pub timeout_ms: Option<u64>,
    pub idle_timeout_ms: Option<u64>,
}
#[derive(Debug, Deserialize, JsonSchema)]
pub struct RenderMapParams {
    pub dmm_path: PathBuf,
    pub z_level: Option<usize>,
    pub output_path: Option<PathBuf>,
    #[serde(default)]
    pub overwrite: bool,
}
#[derive(Debug, Deserialize, JsonSchema)]
pub struct MapInfoParams {
    pub dmm_path: PathBuf,
}
#[derive(Debug, Deserialize, JsonSchema)]
pub struct FindOnMapParams {
    pub dmm_path: PathBuf,
    pub type_path: String,
}
#[derive(Debug, Deserialize, JsonSchema)]
pub struct RunParams {
    pub dmb_path: PathBuf,
    pub port: Option<u16>,
    pub working_directory: Option<PathBuf>,
    #[serde(default)]
    pub daemon_args: Vec<String>,
    pub wait_for: Option<String>,
    #[serde(default)]
    pub wait_regex: bool,
    pub startup_timeout_ms: Option<u64>,
}
#[derive(Debug, Deserialize, JsonSchema)]
pub struct WaitForOutputParams {
    pub pattern: String,
    #[serde(default)]
    pub regex: bool,
    pub timeout_ms: Option<u64>,
}
#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct EmptyParams {}
#[derive(Debug, Deserialize, JsonSchema)]
pub struct TopicParams {
    pub topic: String,
    pub timeout_ms: Option<u64>,
}
