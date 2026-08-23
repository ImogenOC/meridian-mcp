use std::path::Path;

const REQUIRED_DOCUMENTS: &[&str] = &[
    "docs/provenance.md",
    "docs/source-authority.md",
    "docs/compatibility.md",
    "docs/dependency-policy.md",
    "SECURITY.md",
];

#[test]
fn required_trust_documents_exist_and_are_linked() {
    let readme = std::fs::read_to_string("README.md").expect("README should be readable");
    for document in REQUIRED_DOCUMENTS {
        assert!(Path::new(document).is_file(), "missing {document}");
        assert!(
            readme.contains(&format!("]({document})")),
            "README does not link {document}"
        );
    }
}

#[test]
fn local_markdown_links_resolve() {
    for source in ["README.md", "CONTRIBUTING.md", "SECURITY.md"] {
        let text = std::fs::read_to_string(source).expect("Markdown should be readable");
        for target in markdown_link_targets(&text) {
            if target.contains("://") || target.starts_with('#') || target.starts_with("mailto:") {
                continue;
            }
            let target = target.split('#').next().unwrap();
            let resolved = Path::new(source)
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join(target);
            assert!(resolved.exists(), "broken link in {source}: {target}");
        }
    }
}

fn markdown_link_targets(text: &str) -> Vec<String> {
    text.split("](")
        .skip(1)
        .filter_map(|suffix| suffix.split(')').next())
        .map(str::to_owned)
        .collect()
}
