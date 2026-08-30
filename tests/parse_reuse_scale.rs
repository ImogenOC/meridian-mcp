//! Measures the reparse short-circuit against a real, station-sized environment.
//!
//! Ignored by default: it needs a full DreamMaker checkout and takes far longer
//! than a unit test. Point it at one with MERIDIAN_SCALE_DME and run with
//! `cargo test --release --test parse_reuse_scale -- --ignored --nocapture`.
//!
//! This test only reads the environment. Cache invalidation on edit is covered
//! at fixture scale by `an_edited_source_file_forces_a_reparse`, which does not
//! need to mutate a real checkout to prove it.

use meridian_mcp::process_metrics::{process_identity, sample_process, ProcessRole};
use meridian_mcp::result::ToolContent;
use meridian_mcp::state::ServerState;
use meridian_mcp::tools::{call_tool, ToolExecutionContext};
use meridian_mcp::{CapabilityMode, PathPolicy};
use serde_json::{json, Value};
use std::time::Instant;

fn payload(result: &meridian_mcp::result::ToolResult) -> Value {
    let ToolContent::Text { text } = &result.content[0];
    serde_json::from_str(text).expect("tool result should be JSON")
}

#[tokio::test]
#[ignore = "requires a full DreamMaker environment via MERIDIAN_SCALE_DME"]
async fn reusing_a_large_environment_is_orders_of_magnitude_faster() {
    let dme = std::env::var("MERIDIAN_SCALE_DME").expect("set MERIDIAN_SCALE_DME to a .dme path");
    let root = std::path::Path::new(&dme)
        .parent()
        .expect("the .dme should sit in a project root")
        .to_path_buf();
    let context = ToolExecutionContext::new(
        CapabilityMode::Analysis,
        PathPolicy::new(vec![root], Vec::new()).unwrap(),
    );
    let state = ServerState::new();
    let identity = process_identity(std::process::id(), ProcessRole::MeridianMcp).unwrap();
    let memory_before = sample_process(&identity, 0).unwrap();

    let cold_started = Instant::now();
    let cold = call_tool(
        &context,
        &state,
        "dm_parse_environment",
        json!({ "dme_path": dme.clone() }),
    )
    .await
    .unwrap();
    let cold_elapsed = cold_started.elapsed();
    let cold_payload = payload(&cold);
    assert_eq!(cold.is_error, None, "cold parse: {cold_payload}");
    assert_eq!(cold_payload["reused"], false);
    let memory_after = sample_process(&identity, cold_elapsed.as_millis() as u64).unwrap();

    let warm_started = Instant::now();
    let warm = call_tool(
        &context,
        &state,
        "dm_parse_environment",
        json!({ "dme_path": dme.clone() }),
    )
    .await
    .unwrap();
    let warm_elapsed = warm_started.elapsed();
    let warm_payload = payload(&warm);

    println!("types:       {}", cold_payload["total_types"]);
    println!("symbols:     {}", cold_payload["indexed_symbols"]);
    println!(
        "inputs:      {}",
        state.snapshot().await.unwrap().source_inputs().len()
    );
    println!("errors:      {}", cold_payload["error_count"]);
    println!("warnings:    {}", cold_payload["warning_count"]);
    println!("cold parse:  {} ms", cold_elapsed.as_millis());
    println!("warm reuse:  {} ms", warm_elapsed.as_millis());
    println!("memory before: {memory_before:?}");
    println!("memory after:  {memory_after:?}");

    assert_eq!(warm.is_error, None, "warm parse: {warm_payload}");
    assert_eq!(warm_payload["reused"], true);
    assert_eq!(
        warm_payload["state_generation"], cold_payload["state_generation"],
        "reuse must not install a new generation"
    );
    assert_eq!(warm_payload["total_types"], cold_payload["total_types"]);

    // A reuse only re-stats the input list, so it should be far below a parse.
    assert!(
        warm_elapsed * 20 < cold_elapsed,
        "reuse ({warm_elapsed:?}) was not decisively faster than a parse ({cold_elapsed:?})"
    );

    let queries = [
        "dogmos",
        "/datum/controller/subsystem/mapping",
        "native dog library health detection",
        "find references to icon state",
        "air temperature reset",
        "bluespace personal cache",
        "camera network visibility",
        "liquid turf processing",
        "admin technology",
        "move manager path",
    ];
    let mut latencies = Vec::with_capacity(queries.len());
    for query in queries {
        let started = Instant::now();
        let result = call_tool(
            &context,
            &state,
            "dm_search_context",
            json!({ "query": query, "limit": 10, "include_source": false }),
        )
        .await
        .unwrap();
        let elapsed = started.elapsed();
        let body = payload(&result);
        assert_eq!(result.is_error, None, "search {query}: {body}");
        let top_symbol = body["results"][0]["symbol"].as_str().unwrap_or("(none)");
        println!(
            "query={query:?} latency_ms={} candidates={} scored={} top={top_symbol}",
            elapsed.as_millis(),
            body["retrieval"]["candidates_considered"],
            body["retrieval"]["documents_scored"],
        );
        if query == "dogmos" {
            assert!(top_symbol.to_ascii_lowercase().contains("dogmos"));
        }
        if query == "/datum/controller/subsystem/mapping" {
            assert_eq!(top_symbol, query);
        }
        latencies.push(elapsed.as_millis() as u64);
    }
    latencies.sort_unstable();
    println!("query median: {} ms", latencies[latencies.len() / 2]);
    println!("query maximum: {} ms", latencies.last().unwrap());
}
