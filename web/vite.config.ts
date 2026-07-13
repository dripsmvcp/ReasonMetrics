import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

// BASE_PATH lets the GitHub Pages deploy build against /reasonmetrics/ while
// local dev/preview default to "/". Gallery fetches read the resulting
// import.meta.env.BASE_URL instead of hardcoding a root path.
export default defineConfig({
  base: process.env.BASE_PATH ?? "/",
  plugins: [react()],
  test: {
    environment: "node",
    setupFiles: ["./src/test-setup.ts"],
  },
});
