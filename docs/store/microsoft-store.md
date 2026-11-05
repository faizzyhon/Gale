# Microsoft Store submission (MSI)

Listing text is in [listing.md](listing.md). Submit at https://partner.microsoft.com/dashboard → Apps and games →
New product → **MSI or EXE app**. Registering as an individual developer is free.

## Before you submit — required by Store policy 10.2.9

1. **Code-sign the installer and every .exe inside it.** The MSI, `gale.exe`, `gale-nmh.exe`, `yt-dlp.exe`,
   `ffmpeg.exe` and `ffprobe.exe` must be signed with a certificate that chains to the Microsoft Trusted Root
   Program. Self-signed certificates are rejected. Options:
   - **SignPath Foundation** — free code signing for open-source projects (apply with the GitHub repo).
   - An **OV code-signing certificate** from a CA such as Certum, Sectigo or DigiCert (about US$150–300 a year;
     Certum also sells a cheaper open-source developer certificate). Since 2023 these come on a hardware token or
     cloud HSM.
   - Microsoft's own documentation lists Azure Artifact Signing as *not* eligible for Store MSI/EXE submissions.

   Signing also removes the SmartScreen "unknown publisher" warning for people who download the MSI from GitHub.
   `build-msi.sh` produces unsigned files; sign the binaries in `target/msi-stage` before `wixl` runs, then sign the
   finished MSI (`signtool sign /fd sha256 /tr <timestamp-url> /td sha256 …`).

2. **Host the MSI at a versioned HTTPS address that never changes.** Use the GitHub release asset:
   `https://github.com/faizzyhon/Gale/releases/download/v0.4.0/Gale_0.4.0_x64.msi`
   Each update needs a new release and a new URL.

3. **Silent install.** The MSI installs silently with the standard `msiexec /i Gale_0.4.0_x64.msi /qn`. It installs
   for all users, so Windows shows one UAC prompt, which the policy allows. No extra parameters are needed in
   Partner Center for an MSI.

## Product setup

| Field | Value |
|---|---|
| Product name (reserve) | Gale Download Manager |
| Package URL | the release URL above |
| Architecture | x64 |
| Installer parameters | none (MSI) |
| App type | Win32 (MSI) |

## Properties

| Field | Value |
|---|---|
| Category | Utilities & tools |
| Subcategory | File managers (or "Other" if file managers is not offered) |
| Privacy policy URL | https://github.com/faizzyhon/Gale/blob/main/PRIVACY.md |
| Website | https://faizzyhon.com |
| Support contact | https://github.com/faizzyhon/Gale/issues |
| Product declarations | Not a game; does not access, collect or transmit personal information to the developer |
| System requirements | Windows 10 version 1809+ / Windows 11, x64; 4 GB RAM recommended; internet connection |
| Supports pen/touch | No special support (mouse and keyboard) |

## Age ratings (IARC questionnaire)

Choose category **Utility / productivity / communication / other**, then answer honestly:

- Violence, sexual content, language, gambling, controlled substances: **No**
- Does the app let users interact or exchange content with other users? **No**
- Does the app share the user's location with others? **No**
- Does the app allow purchases of digital goods? **No**
- Does the app give access to unrestricted internet content? **Yes** — it downloads any file or video address the
  user gives it. (This is shown as an "Unrestricted Internet" notice next to the rating.)

## Store listing (English)

| Field | Value |
|---|---|
| Product name | Gale Download Manager |
| Short description | "Short description" in listing.md |
| Description | "Long description" in listing.md (leave out the extension download line) |
| What's new in this version | "What's new in 0.4.0" in listing.md |
| Product features (up to 20) | lines from "Feature list" in listing.md |
| Screenshots | `assets/microsoft/screenshot-1…5.png` (1920×1080) |
| Store logo (1:1) | `assets/microsoft/store-logo-300x300.png` |
| Hero / "Super hero art" (16:9) | `assets/microsoft/hero-1920x1080.png` |
| Keywords (up to 7) | download manager, downloader, download accelerator, resume downloads, video downloader, open source, free |
| Copyright | © 2026 Muhammad Faizan |
| Developed by | Muhammad Faizan |
| Additional license terms | https://github.com/faizzyhon/Gale/blob/main/TERMS.md |

## Pricing and availability

Free, all markets, public.

## Review risk to know about

Store policy 10.1.6 says a product that uses content from a third-party service must be permitted to do so by that
service's terms, and YouTube's terms do not allow downloading. The Store may ask for the YouTube support to be
removed before approval. If that happens, the Chrome Web Store approach in this repo (switch the feature off for
YouTube in a store build) can be applied to the app as well.

## Notes for certification (optional field)

> Gale is an open-source download manager (MIT): https://github.com/faizzyhon/Gale.
> To test: Add URL → paste any direct file link (for example a Linux ISO) → Start download. The optional browser
> extension is not required for testing. The installer asks for administrator approval and installs for all users.

---

# winget (Windows Package Manager)

Manifests are in [winget/](winget/). After the v0.4.0 GitHub release exists with `Gale_0.4.0_x64.msi` attached:

1. Check `InstallerSha256` matches the uploaded file (`certutil -hashfile Gale_0.4.0_x64.msi SHA256`).
2. Validate: `winget validate --manifest docs\store\winget\0.4.0`
3. Test: `winget install --manifest docs\store\winget\0.4.0`
4. Submit with `wingetcreate submit docs\store\winget\0.4.0` (or open a pull request to
   https://github.com/microsoft/winget-pkgs under `manifests/m/MuhammadFaizan/Gale/0.4.0/`).

winget does not require code signing, but unsigned installers can be held for extra review.
