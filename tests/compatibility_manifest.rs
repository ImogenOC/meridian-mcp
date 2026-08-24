use serde_json::Value;
use std::collections::HashSet;

fn manifest() -> Value {
    let text = std::fs::read_to_string("tests/compatibility/meridian-rift.json")
        .expect("Meridian-Rift compatibility manifest must be checked in");
    serde_json::from_str(&text).expect("compatibility manifest must be valid JSON")
}

#[test]
fn manifest_is_versioned_unique_bounded_and_covers_analysis_contract() {
    let manifest = manifest();
    assert_eq!(manifest["schema_version"], 1);

    let sections = [
        "types",
        "procs",
        "vars",
        "type_lists",
        "symbol_searches",
        "context_searches",
        "definitions",
    ];
    for section in sections {
        let cases = manifest[section]
            .as_array()
            .unwrap_or_else(|| panic!("{section} must be an array"));
        assert!(!cases.is_empty(), "{section} must not be empty");
        let unique = cases.iter().map(Value::to_string).collect::<HashSet<_>>();
        assert_eq!(unique.len(), cases.len(), "{section} has duplicate cases");
    }

    for case in manifest["context_searches"].as_array().unwrap() {
        let top = case["top"]
            .as_u64()
            .expect("context top must be an integer");
        assert!((1..=50).contains(&top), "context top is out of bounds");
    }

    for suffix in collect_strings_named(&manifest, "file_suffix") {
        assert!(!suffix.is_empty());
        assert!(!suffix.starts_with('/') && !suffix.starts_with('\\'));
        assert!(!suffix.contains(':'));
        assert!(!suffix.split('/').any(|component| component == ".."));
        assert!(
            !suffix.contains('\\'),
            "suffixes must use repository separators"
        );
    }

    let covered_tools = [
        "dm_parse_environment",
        "dm_get_type",
        "dm_get_proc",
        "dm_get_var",
        "dm_list_types",
        "dm_search_symbols",
        "dm_search_context",
        "dm_get_definition",
    ];
    assert_eq!(
        manifest["covered_tools"],
        serde_json::to_value(covered_tools).unwrap()
    );
}

fn collect_strings_named<'a>(value: &'a Value, name: &str) -> Vec<&'a str> {
    let mut output = Vec::new();
    match value {
        Value::Array(values) => {
            for value in values {
                output.extend(collect_strings_named(value, name));
            }
        }
        Value::Object(values) => {
            for (key, value) in values {
                if key == name {
                    output.push(value.as_str().expect("named value must be a string"));
                }
                output.extend(collect_strings_named(value, name));
            }
        }
        _ => {}
    }
    output
}
