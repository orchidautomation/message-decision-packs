import type { NextConfig } from "next";
import { withWorkflow } from "workflow/next";

const nextConfig: NextConfig = {
  typedRoutes: false
};

export default withWorkflow(nextConfig);
