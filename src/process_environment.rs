use std::ffi::OsString;

const RUNTIME_ENVIRONMENT_NAMES: &[&str] = &[
    "SystemRoot",
    "WINDIR",
    "TEMP",
    "TMP",
    "USERPROFILE",
    "APPDATA",
    "LOCALAPPDATA",
    "ProgramData",
    "PATH",
    "LD_LIBRARY_PATH",
];

pub fn minimal_runtime_environment() -> Vec<(String, OsString)> {
    RUNTIME_ENVIRONMENT_NAMES
        .iter()
        .filter_map(|name| std::env::var_os(name).map(|value| ((*name).to_owned(), value)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn environment_allowlist_excludes_profiler_configuration() {
        assert!(!RUNTIME_ENVIRONMENT_NAMES.contains(&"UTRACY_BIND_ADDRESS"));
        assert!(!RUNTIME_ENVIRONMENT_NAMES.contains(&"UTRACY_BIND_PORT"));
    }
}
