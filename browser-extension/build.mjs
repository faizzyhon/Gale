// Bundles each entry point and copies static files into dist/ (load dist/ as the unpacked extension).
import { build } from "esbuild";
import { cpSync, mkdirSync, rmSync } from "node:fs";

const tests = process.argv.includes("--tests");
if (tests) {
  rmSync("dist-tests", { recursive: true, force: true });
  await build({ entryPoints: ["tests/rules.test.ts"], bundle: true, platform: "node", format: "esm", outdir: "dist-tests" });
} else {
  rmSync("dist", { recursive: true, force: true });
  mkdirSync("dist", { recursive: true });
  await build({
    entryPoints: {
      background: "src/background/index.ts",
      content: "src/content/media.ts",
      popup: "src/popup/popup.ts",
      options: "src/options/options.ts",
    },
    bundle: true,
    format: "iife", // content scripts cannot be modules
    target: "chrome116",
    outdir: "dist",
    minify: true,
    legalComments: "none",
  });
  for (const f of ["manifest.json", "src/popup/popup.html", "src/options/options.html", "src/ui.css"]) cpSync(f, `dist/${f.split("/").pop()}`);
  cpSync("icons", "dist/icons", { recursive: true });
}
