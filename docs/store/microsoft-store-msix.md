# Microsoft Store submission as MSIX (no certificate needed)

The Store rejects unsigned MSI/EXE installers (policy 10.2.9). An MSIX package is signed by the Store itself, so this
route needs no code-signing certificate. The listing text, images and age rating from the MSI submission are reused.

## 1. Swap the product in Partner Center

A product type (MSI/EXE vs MSIX) cannot be changed, and a name can belong to one product only.

1. Open the existing **Gale Download Manager** (MSI/EXE) product → **Product management** → **Delete product**
   (or remove the reserved name from it).
2. **Apps and games** → **New product** → **MSIX or PWA app** → reserve **Gale Download Manager**.
3. Open **Product management** → **Product identity** and copy:
   - `Package/Identity/Name` (for example `12345Faizzyhon.GaleDownloadManager`)
   - `Package/Identity/Publisher` (starts with `CN=`)
   - `Package/Properties/PublisherDisplayName` (for example `Faizzyhon`)

## 2. Build the package (on Windows)

1. Install the [Windows SDK](https://developer.microsoft.com/windows/downloads/windows-sdk/) (only "Windows SDK
   Signing Tools for Desktop Apps" and "Windows App Certification Kit" are needed; MakeAppx comes with them).
2. Unzip `Gale-MSIX-kit-<version>.zip`.
3. In that folder:

   ```
   build-msix.cmd "<Package/Identity/Name>" "<Package/Identity/Publisher>" "<PublisherDisplayName>" 0.7.0.0
   ```

   The result is `Gale_0.7.0.0_x64.msix`. The fourth number of the version must stay 0 (Store rule); raise the first
   three for each update, for example `0.5.1.0`.

### Optional: try it before uploading

Turn on **Developer Mode** (Settings → System → For developers), uninstall the MSI version of Gale, then run
`test-msix.cmd` after `build-msix.cmd`. Gale appears in the Start menu as the Store version. Check that it opens,
downloads a file, and that the browser extension shows "Connected". Remove it again with
`Get-AppxPackage *Gale* | Remove-AppxPackage` in PowerShell.

The Windows App Certification Kit (Start → "Windows App Cert Kit") can test the .msix the way the Store does.

## 3. Submission

| Page | What to enter |
|---|---|
| Pricing and availability | Free, all markets |
| Properties | same as the MSI submission (category, privacy policy, support, system requirements) |
| Age ratings | same answers as before (All other app types; all "No") |
| Packages | upload `Gale_0.7.0.0_x64.msix` |
| Store listings | same text and images as before (`docs/store/assets/microsoft/`) |
| Submission options → Notes for certification | the MSI notes, but step 1 becomes "Install from the Store package" |

### Restricted capabilities

Partner Center asks why the package needs two restricted capabilities. Paste:

> **runFullTrust:** Gale is a classic Win32 desktop application (download manager) packaged as MSIX.
>
> **unvirtualizedResources:** Gale's optional Chrome/Edge extension talks to the app through a native messaging
> host. Gale registers that host under HKCU\Software\Google\Chrome\NativeMessagingHosts and
> HKCU\Software\Microsoft\Edge\NativeMessagingHosts, which the browsers read from the real registry, and the helper
> (gale-nmh.exe) is started by the browser outside the package, so it must find Gale's data in the real
> %LOCALAPPDATA%. Without unvirtualized registry and file system writes the browser extension cannot connect.
> Gale writes only its own keys and its own folder (%LOCALAPPDATA%\com.faizzyhon.gale).

## How the Store version differs from the MSI

- Windows installs, updates and removes it. "Start with Windows" is the package's startup task: users turn it off
  in Windows Settings → Apps → Startup (Gale's own setting is hidden).
- Requires Windows 10 version 2004 or later (the MSI supports 1809+).
- WebView2 is not installed by the package; Windows 11 and current Windows 10 have it.
- Uninstalling leaves the two browser registry keys pointing at the removed helper (harmless; the extension then
  shows "Gale is not installed").
