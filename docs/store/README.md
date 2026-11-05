# Publishing Gale

Everything needed to list Gale in stores and on download sites.

| File | For |
|---|---|
| [listing.md](listing.md) | Name, descriptions, features, keywords, release notes, developer bio, requirements — copy from here |
| [chrome-web-store.md](chrome-web-store.md) | Chrome Web Store and Edge Add-ons: store build, permission justifications, privacy answers |
| [microsoft-store.md](microsoft-store.md) | Microsoft Store (MSI) and winget: signing, age rating, listing fields |
| [download-sites.md](download-sites.md) | GitHub Releases, Softpedia, Uptodown, FileHorse, MajorGeeks, SourceForge, AlternativeTo |
| [winget/](winget/) | winget manifests for v0.5.0 |
| [assets/](assets/) | Screenshots, promo tiles, store logo |
| [../../PRIVACY.md](../../PRIVACY.md) | Privacy policy (stores ask for its URL) |
| [../../TERMS.md](../../TERMS.md) | Terms of use |

## Order of work

1. Push the code and create the GitHub release `v0.5.0` with the setup .exe, the MSI, their `.sha256` files and the extension zip attached.
   The privacy policy, terms and download links in every listing point to GitHub, so this comes first.
2. Chrome Web Store, then Edge Add-ons. After each is approved, add its extension ID to the app (see
   chrome-web-store.md step 2) and publish an updated MSI.
3. winget (free, no signing needed).
4. Download sites.
5. Microsoft Store once the installer is code-signed.
