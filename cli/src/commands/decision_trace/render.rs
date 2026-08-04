use super::{DecisionTrace, MAX_MERMAID_BYTES, TraceGraph, truncate_utf8};

pub(crate) fn render_mermaid(trace: &DecisionTrace) -> String {
    let mut output = String::from("flowchart TD\n");
    render_graph(&mut output, "d", &trace.designed_graph, "designed");
    render_graph(&mut output, "o", &trace.observed_path, "observed");
    output.push_str("  classDef designed fill:#eef2ff,stroke:#6366f1,color:#111827\n");
    output.push_str("  classDef observed fill:#ecfdf5,stroke:#059669,color:#111827\n");
    output.push_str("  classDef blocked fill:#fef2f2,stroke:#dc2626,color:#111827\n");
    if output.len() > MAX_MERMAID_BYTES {
        const TRUNCATION_NODE: &str = "\n  truncated[\"Projection output truncated\"]\n";
        output = truncate_utf8(
            &output,
            MAX_MERMAID_BYTES.saturating_sub(TRUNCATION_NODE.len()),
        )
        .0;
        output.push_str(TRUNCATION_NODE);
    }
    output
}

fn render_graph(output: &mut String, prefix: &str, graph: &TraceGraph, default_class: &str) {
    for node in &graph.nodes {
        let id = format!("{prefix}_{}", node.id.replace('-', "_"));
        let class = if node.state == "blocked" {
            "blocked"
        } else {
            default_class
        };
        output.push_str(&format!(
            "  {id}[\"{}\"]:::{class}\n",
            mermaid_escape(&node.label)
        ));
    }
    for edge in &graph.edges {
        let from = format!("{prefix}_{}", edge.from.replace('-', "_"));
        let to = format!("{prefix}_{}", edge.to.replace('-', "_"));
        output.push_str(&format!(
            "  {from} -->|{}| {to}\n",
            mermaid_escape(edge.kind)
        ));
    }
}

fn mermaid_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('%', "&#37;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('[', "&#91;")
        .replace(']', "&#93;")
        .replace('{', "&#123;")
        .replace('}', "&#125;")
        .replace('|', "&#124;")
        .replace('`', "&#96;")
}
