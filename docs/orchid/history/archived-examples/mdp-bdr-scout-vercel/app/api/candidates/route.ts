import sampleRow from "../../../samples/candidate-ledger-row.json";

export async function GET(): Promise<Response> {
  return Response.json({ candidates: [sampleRow], source: "sample" });
}
