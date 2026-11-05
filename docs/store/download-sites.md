# Download sites and software directories

Use the copy in [listing.md](listing.md) and the 1920×1080 screenshots in `assets/microsoft/`. All of these take the
GitHub release link as the download, so updating them means editing the version and link.

| Site | Where to submit (submission pages move; use the site's footer link if a link has changed) | Notes |
|---|---|---|
| GitHub Releases | https://github.com/faizzyhon/Gale/releases/new | Tag `v0.5.0`; attach `Gale_Setup_0.5.0.exe`, `Gale_0.5.0_x64.msi`, their `.sha256` files, and `Gale-Chrome-Extension-0.5.0.zip`; paste "What's new". Every other site links here. |
| winget | see [microsoft-store.md](microsoft-store.md#winget-windows-package-manager) | Lets people run `winget install MuhammadFaizan.Gale`. |
| Softpedia | https://www.softpedia.com — "Submit program" link in the footer | Free. Mark as "Freeware / Open Source". |
| Uptodown | https://www.uptodown.com — "Developers" / "Upload your app" in the footer | Free developer account; upload or link the MSI. |
| FileHorse | https://www.filehorse.com — "Submit Software" in the footer | Free submission form. |
| MajorGeeks | https://www.majorgeeks.com — "Submit Software" page | Email submission; include the release link. |
| SourceForge | https://sourceforge.net/create/ | Can mirror GitHub releases automatically ("Import GitHub project"). |
| AlternativeTo | https://alternativeto.net/software/new/ | Add Gale and list it as an alternative to other download managers. Category: Download Manager. |
| Neowin software | https://www.neowin.net — news tip / contact form | Submit through the news tip form with release notes. |

## Fields most sites ask for

| Field | Value |
|---|---|
| Program name | Gale Download Manager |
| Version | 0.5.0 |
| License | Freeware / Open Source (MIT) |
| Price | Free |
| OS | Windows 10, Windows 11 (64-bit) |
| File | `Gale_Setup_0.5.0.exe` (~160 MB, includes yt-dlp and FFmpeg; installs WebView2 if missing) |
| Download URL | https://github.com/faizzyhon/Gale/releases/download/v0.5.0/Gale_Setup_0.5.0.exe |
| Homepage | https://github.com/faizzyhon/Gale |
| Publisher | Muhammad Faizan |
| Publisher website | https://faizzyhon.com |
| Contact email | faizzyhon@gmail.com |
| Category | Internet → Download Managers |
| Ads / bundled offers | None |
| Requires admin | Yes, for installing (installs for all users in Program Files); Gale itself runs without admin |

## Tips

- Many sites scan uploads with VirusTotal. An unsigned MSI with `yt-dlp.exe` inside can get one or two false
  positives; code signing (see microsoft-store.md) reduces this. If a site flags it, link the VirusTotal report and
  the source code in your reply.
- Keep the same short description everywhere so search results look consistent.
