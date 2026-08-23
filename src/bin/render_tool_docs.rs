fn main() -> anyhow::Result<()> {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "docs/tool-contracts.md".into());
    std::fs::write(
        path,
        meridian_mcp::render_tool_reference(meridian_mcp::all_contracts()),
    )?;
    Ok(())
}
