export default function RunsPage() {
  return (
    <main style={{ fontFamily: "system-ui, sans-serif", margin: "3rem auto", maxWidth: 920, padding: "0 1.5rem" }}>
      <h1>Scout runs</h1>
      <p>Wire this page to Neon `scout_runs` once the storage adapter is enabled.</p>
      <pre>{`GET /api/cron/scout -> scoutCycleWorkflow -> ledger rows`}</pre>
    </main>
  );
}
