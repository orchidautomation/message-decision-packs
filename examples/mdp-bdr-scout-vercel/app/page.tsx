const cards = [
  ["Strategize", "An mdp.source-strategy.v0 artifact turns ICP and pack primitives into safe search targets."],
  ["Discover", "Exa-first search over companies, people, news, and public triggers."],
  ["Extract", "Firecrawl for clean pages; Apify for Store actors and hard-site datasets."],
  ["Decide", "MDP fit, routing, brief context, and claim checks stay application-owned."],
  ["Ledger", "Normalized evidence-backed rows ready for CRM sync review."]
];

export default function HomePage() {
  return (
    <main style={{ fontFamily: "system-ui, sans-serif", margin: "3rem auto", maxWidth: 920, padding: "0 1.5rem" }}>
      <p style={{ color: "#666", textTransform: "uppercase", letterSpacing: "0.08em" }}>Message Decision Packs</p>
      <h1>BDR Scout powered by MDP</h1>
      <p style={{ fontSize: "1.1rem", lineHeight: 1.6 }}>
        A Vercel-first scout that wakes on a schedule, gathers evidence, runs MDP-owned fit and brief gates,
        and appends normalized lead ledger rows without sending outreach by default.
      </p>
      <section style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(190px, 1fr))", gap: "1rem", marginTop: "2rem" }}>
        {cards.map(([title, body]) => (
          <article key={title} style={{ border: "1px solid #ddd", borderRadius: 12, padding: "1rem" }}>
            <h2 style={{ fontSize: "1rem" }}>{title}</h2>
            <p style={{ color: "#555", lineHeight: 1.5 }}>{body}</p>
          </article>
        ))}
      </section>
    </main>
  );
}
