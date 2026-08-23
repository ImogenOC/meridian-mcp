use crate::CapabilityMode;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ToolEffects {
    pub reads_files: bool,
    pub writes_files: bool,
    pub spawns_process: bool,
    pub network_loopback: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SupportLevel {
    Verified,
    Provisional,
    Experimental,
    Unsupported,
}

#[derive(Clone, Copy, Debug)]
pub struct ToolContract {
    pub name: &'static str,
    pub summary: &'static str,
    pub mode: CapabilityMode,
    pub effects: ToolEffects,
    pub support: SupportLevel,
    pub timeout_ms: Option<u64>,
    pub max_output_bytes: usize,
}

const READ: ToolEffects = ToolEffects {
    reads_files: true,
    writes_files: false,
    spawns_process: false,
    network_loopback: false,
};
const MEMORY: ToolEffects = ToolEffects {
    reads_files: false,
    writes_files: false,
    spawns_process: false,
    network_loopback: false,
};
const COMPILE: ToolEffects = ToolEffects {
    reads_files: true,
    writes_files: true,
    spawns_process: true,
    network_loopback: false,
};
const RENDER: ToolEffects = ToolEffects {
    reads_files: true,
    writes_files: true,
    spawns_process: false,
    network_loopback: false,
};
const RUNTIME: ToolEffects = ToolEffects {
    reads_files: true,
    writes_files: true,
    spawns_process: true,
    network_loopback: true,
};
const TOPIC: ToolEffects = ToolEffects {
    reads_files: false,
    writes_files: false,
    spawns_process: false,
    network_loopback: true,
};

macro_rules! contract {
    ($name:literal, $summary:literal, $mode:ident, $effects:ident, $support:ident, $timeout:expr, $max:expr) => {
        ToolContract {
            name: $name,
            summary: $summary,
            mode: CapabilityMode::$mode,
            effects: $effects,
            support: SupportLevel::$support,
            timeout_ms: $timeout,
            max_output_bytes: $max,
        }
    };
}

static CONTRACTS: &[ToolContract] = &[
    contract!(
        "dm_parse_environment",
        "Parse and atomically index a DreamMaker environment.",
        Analysis,
        READ,
        Provisional,
        None,
        1_048_576
    ),
    contract!(
        "dm_get_type",
        "Inspect an exact DreamMaker type.",
        Analysis,
        MEMORY,
        Provisional,
        None,
        1_048_576
    ),
    contract!(
        "dm_get_proc",
        "Inspect exact proc implementations and source excerpts.",
        Analysis,
        READ,
        Provisional,
        None,
        1_048_576
    ),
    contract!(
        "dm_get_var",
        "Inspect an exact DreamMaker variable.",
        Analysis,
        MEMORY,
        Provisional,
        None,
        262_144
    ),
    contract!(
        "dm_list_types",
        "List parsed types under an optional prefix.",
        Analysis,
        MEMORY,
        Provisional,
        None,
        1_048_576
    ),
    contract!(
        "dm_search_symbols",
        "Search parsed symbol names.",
        Analysis,
        MEMORY,
        Provisional,
        None,
        1_048_576
    ),
    contract!(
        "dm_search_context",
        "Rank parsed symbols and source context deterministically.",
        Analysis,
        READ,
        Provisional,
        None,
        1_048_576
    ),
    contract!(
        "dm_check_errors",
        "Run SpacemanDMM DreamChecker diagnostics.",
        Analysis,
        MEMORY,
        Provisional,
        None,
        1_048_576
    ),
    contract!(
        "dm_get_definition",
        "Locate an exact parsed definition.",
        Analysis,
        MEMORY,
        Provisional,
        None,
        262_144
    ),
    contract!(
        "dm_map_info",
        "Read DMM/TGM dimensions and atom statistics.",
        Analysis,
        READ,
        Provisional,
        None,
        1_048_576
    ),
    contract!(
        "dm_find_on_map",
        "Find exact type instances in a DMM/TGM map.",
        Analysis,
        READ,
        Provisional,
        None,
        1_048_576
    ),
    contract!(
        "dm_compile",
        "Run an allowlisted DreamMaker compiler gate.",
        Development,
        COMPILE,
        Provisional,
        Some(1_800_000),
        1_048_576
    ),
    contract!(
        "dm_render_map",
        "Render a contained DMM/TGM map output.",
        Development,
        RENDER,
        Provisional,
        None,
        262_144
    ),
    contract!(
        "dm_run",
        "Start a contained DreamDaemon program on loopback.",
        Development,
        RUNTIME,
        Provisional,
        Some(300_000),
        1_048_576
    ),
    contract!(
        "dm_wait_for_output",
        "Wait for bounded server-owned DreamDaemon output.",
        Development,
        MEMORY,
        Provisional,
        Some(300_000),
        1_048_576
    ),
    contract!(
        "dm_stop",
        "Stop the server-owned DreamDaemon process.",
        Development,
        MEMORY,
        Provisional,
        None,
        262_144
    ),
    contract!(
        "dm_status",
        "Inspect server-owned DreamDaemon state.",
        Development,
        MEMORY,
        Provisional,
        None,
        1_048_576
    ),
    contract!(
        "dm_topic",
        "Call world.Topic on the loopback game server.",
        Development,
        TOPIC,
        Provisional,
        Some(60_000),
        262_144
    ),
];

pub fn all_contracts() -> &'static [ToolContract] {
    CONTRACTS
}

pub fn contracts_for(mode: CapabilityMode) -> Vec<&'static ToolContract> {
    CONTRACTS
        .iter()
        .filter(|contract| {
            contract.support != SupportLevel::Unsupported
                && (contract.mode == CapabilityMode::Analysis
                    || mode == CapabilityMode::Development)
        })
        .collect()
}

pub fn render_tool_reference(contracts: &[ToolContract]) -> String {
    let mut contracts = contracts.to_vec();
    contracts.sort_by_key(|contract| contract.name);
    let mut output = String::from("# Tool contracts\n\nGenerated from `src/contracts.rs`; do not edit by hand.\n\n| Tool | Mode | Support | Effects | Timeout ms | Max output bytes | Summary |\n| --- | --- | --- | --- | ---: | ---: | --- |\n");
    for contract in contracts {
        let mut effects = Vec::new();
        if contract.effects.reads_files {
            effects.push("read");
        }
        if contract.effects.writes_files {
            effects.push("write");
        }
        if contract.effects.spawns_process {
            effects.push("process");
        }
        if contract.effects.network_loopback {
            effects.push("loopback");
        }
        output.push_str(&format!(
            "| `{}` | {:?} | {:?} | {} | {} | {} | {} |\n",
            contract.name,
            contract.mode,
            contract.support,
            if effects.is_empty() {
                "memory".to_owned()
            } else {
                effects.join(", ")
            },
            contract
                .timeout_ms
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".into()),
            contract.max_output_bytes,
            contract.summary
        ));
    }
    output
}
