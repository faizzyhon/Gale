# Building the Gale Firefox add-on from source

Requirements: Node.js 22, pnpm 10. Any OS.

```
pnpm install --frozen-lockfile
pnpm build:firefox
```

The add-on is written to `dist-firefox/`; its files match the uploaded package. The code is TypeScript in `src/`,
bundled and minified with esbuild (`build.mjs`). No code is downloaded at runtime; the add-on only talks to the
local Gale app through native messaging (host `com.gale.nmh`).
