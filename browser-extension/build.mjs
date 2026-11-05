// Bundles each entry point and copies static files into dist/ (load dist/ as the unpacked extension).
import { build } from "esbuild";
import { cpSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";

const tests = process.argv.includes("--tests");
const store = process.argv.includes("--store"); // Chrome Web Store build, see src/shared/store.ts
const out = store ? "dist-store" : "dist";
if (tests) {
  rmSync("dist-tests", { recursive: true, force: true });
  await build({ entryPoints: ["tests/rules.test.ts"], bundle: true, platform: "node", format: "esm", outdir: "dist-tests", define: { __STORE__: "false" } });
} else {
  rmSync(out, { recursive: true, force: true });
  mkdirSync(out, { recursive: true });
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
    outdir: out,
    define: { __STORE__: String(store) },
    minify: true,
    legalComments: "none",
  });
  // The Chrome Web Store assigns its own ID and rejects a manifest "key"; local builds keep it for a fixed ID.
  const manifest = JSON.parse(readFileSync("manifest.json", "utf8"));
  if (store) delete manifest.key;
  writeFileSync(`${out}/manifest.json`, JSON.stringify(manifest, null, 2));
  for (const f of ["src/popup/popup.html", "src/options/options.html", "src/ui.css"]) cpSync(f, `${out}/${f.split("/").pop()}`);
  cpSync("icons", `${out}/icons`, { recursive: true });
}
