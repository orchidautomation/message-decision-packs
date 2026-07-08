export async function writeBriefArtifact(_key: string, _body: string): Promise<string> {
  throw new Error("Vercel Blob persistence is not wired in the offline scaffold. Add @vercel/blob writes after credential policy is set.");
}
