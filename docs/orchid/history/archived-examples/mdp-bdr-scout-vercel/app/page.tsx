const steps = [
  ["01", "Strategy", "Load the Profound MDP and .mdp/source-strategy.json before provider discovery."],
  ["02", "Discovery", "Use Exa-first public search, Firecrawl for accepted URLs, and Apify only when an operator approves the actor."],
  ["03", "Decision", "Run MDP fit, route, claim checks, and create-brief before any copy or CRM handoff."],
  ["04", "Ledger", "Append evidence-backed rows with outreach and CRM sync disabled by default."]
];

export default function HomePage() {
  return (
    <main style={{ background: "#fff", color: "#000", fontFamily: "Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif", margin: "4rem auto", maxWidth: 960, padding: "0 1.5rem" }}>
      <p style={{ color: "#666", fontSize: 12, fontWeight: 600, letterSpacing: "0.14em", marginBottom: "1.25rem", textTransform: "uppercase" }}>Message Decision Packs</p>
      <h1 style={{ fontSize: "clamp(2.5rem, 7vw, 5.5rem)", letterSpacing: "-0.07em", lineHeight: 0.92, margin: 0, maxWidth: 820 }}>BDR Scout, grounded by MDP.</h1>
      <p style={{ color: "#444", fontSize: "1.125rem", lineHeight: 1.7, marginTop: "1.5rem", maxWidth: 680 }}>
        A Vercel-first scout that wakes on a schedule, gathers public evidence, runs MDP-owned fit and brief gates,
        and writes normalized lead ledger rows without sending outreach by default.
      </p>
      <div style={{ borderTop: "1px solid #eaeaea", marginTop: "3rem" }}>
        {steps.map(([number, title, body]) => (
          <div key={title} style={{ alignItems: "baseline", borderBottom: "1px solid #eaeaea", display: "grid", gap: "1rem", gridTemplateColumns: "56px minmax(120px, 180px) 1fr", padding: "1.1rem 0" }}>
            <span style={{ color: "#888", fontVariantNumeric: "tabular-nums" }}>{number}</span>
            <strong style={{ fontSize: "0.95rem" }}>{title}</strong>
            <span style={{ color: "#444", lineHeight: 1.6 }}>{body}</span>
          </div>
        ))}
      </div>
      <p style={{ color: "#666", fontSize: "0.9rem", marginTop: "2rem" }}>
        Recommended pack: <code>examples/profound-gtm-vetting/.mdp</code>. Brief command: <code>mdp --json brief --context</code>.
      </p>
    </main>
  );
}
