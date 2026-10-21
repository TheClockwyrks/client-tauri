// The shell script's tests, each `test/*.test.ts` run against a happy-dom
// window it builds itself, so a test chooses the page's URL.
//
// Given CI_GATE_ARTIFACTS (the typescript gate sets it), the run writes the
// JUnit report and the coverage summary into that directory, as ci/README.md
// names them; without it, it writes neither.
import { defineConfig } from "vitest/config";

const artifacts = process.env.CI_GATE_ARTIFACTS;

export default defineConfig({
  test: {
    environment: "node",
    include: ["test/**/*.test.ts"],
    ...(artifacts
      ? {
          reporters: ["default", "junit"],
          outputFile: { junit: `${artifacts}/junit.xml` },
          coverage: {
            enabled: true,
            provider: "v8",
            reporter: ["json-summary"],
            reportsDirectory: artifacts,
            include: ["src/**/*.cjs"],
          },
        }
      : {}),
  },
});
