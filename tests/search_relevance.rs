use meridian_mcp::result::ToolContent;
use meridian_mcp::state::ServerState;
use meridian_mcp::tools::{call_tool, ToolExecutionContext};
use meridian_mcp::{CapabilityMode, PathPolicy};
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize)]
struct RelevanceSet {
    schema_version: u32,
    judgments: Vec<Judgment>,
}

#[derive(Deserialize)]
struct Judgment {
    category: String,
    query: String,
    relevant_symbols: Vec<String>,
    required_first: bool,
}

fn payload(result: &meridian_mcp::result::ToolResult) -> Value {
    let ToolContent::Text { text } = &result.content[0];
    serde_json::from_str(text).expect("tool result should be JSON")
}

#[tokio::test]
async fn lexical_retrieval_meets_the_owned_relevance_judgments() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/search");
    let relevance: RelevanceSet = serde_json::from_slice(
        &std::fs::read(root.join("relevance.json")).expect("relevance fixture should exist"),
    )
    .expect("relevance fixture should be valid JSON");
    assert_eq!(relevance.schema_version, 1);

    let context = ToolExecutionContext::new(
        CapabilityMode::Analysis,
        PathPolicy::new(vec![root.clone()], Vec::new()).unwrap(),
    );
    let state = ServerState::new();
    let parsed = call_tool(
        &context,
        &state,
        "dm_parse_environment",
        json!({"dme_path": root.join("fixture.dme")}),
    )
    .await
    .unwrap();
    assert_eq!(payload(&parsed)["success"], true);

    let mut reciprocal_ranks = Vec::new();
    let mut natural_recalled = 0_usize;
    let mut natural_count = 0_usize;
    for judgment in relevance.judgments {
        let result = call_tool(
            &context,
            &state,
            "dm_search_context",
            json!({"query": judgment.query, "limit": 10, "include_source": false}),
        )
        .await
        .unwrap();
        let body = payload(&result);
        assert_eq!(body["retrieval"]["mode"], "lexical");
        let symbols = body["results"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|result| result["symbol"].as_str())
            .collect::<Vec<_>>();
        let first_relevant = symbols.iter().position(|symbol| {
            judgment
                .relevant_symbols
                .iter()
                .any(|relevant| relevant == *symbol)
        });
        if judgment.required_first {
            assert_eq!(first_relevant, Some(0), "query: {}", judgment.query);
        }
        match judgment.category.as_str() {
            "exact_identifier" => reciprocal_ranks.push(
                first_relevant
                    .map(|rank| 1.0 / (rank + 1) as f64)
                    .unwrap_or(0.0),
            ),
            "natural_language" => {
                natural_count += 1;
                natural_recalled += usize::from(first_relevant.is_some());
            }
            category => panic!("unknown relevance category: {category}"),
        }
    }

    let exact_identifier_mrr = reciprocal_ranks.iter().sum::<f64>() / reciprocal_ranks.len() as f64;
    let natural_language_recall_at_10 = natural_recalled as f64 / natural_count as f64;
    assert_eq!(exact_identifier_mrr, 1.0);
    assert_eq!(natural_language_recall_at_10, 1.0);
}
