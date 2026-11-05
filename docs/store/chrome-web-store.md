# Chrome Web Store and Edge Add-ons submission

Listing text is in [listing.md](listing.md). This file has the store-specific fields.

## 1. Build the store package

```
cd browser-extension
pnpm install
pnpm run build:store          # → browser-extension/dist-store/
cd dist-store && zip -r ../../Gale-Chrome-Web-Store-0.5.0.zip . && cd ../..
```

The store build is the normal extension with two differences:

- **No YouTube video downloads.** The Chrome Web Store rejects extensions that download YouTube media
  (violation "Blue Zinc": *facilitating unauthorized access to or download of copyrighted content or media,
  specifically, YouTube*). In the store build the Download button and "Download video on this page" are switched off
  on youtube.com, youtu.be and googlevideo.com. Everything else is identical. The app itself, and the extension
  shipped inside the MSI, are unchanged.
- **No `key` in manifest.json.** The store gives the extension its own ID.

## 2. After the first upload: allow the store ID

The Gale app only talks to extension IDs it knows. When the store shows your new item ID (32 letters), add it in
both places, then rebuild the MSI:

1. `crates/gale-nmh/src/main.rs` → `ALLOWED_ORIGINS`
2. `scripts/installer/com.gale.nmh.json` → `allowed_origins`

```
"allowed_origins": [
  "chrome-extension://abiednbonckeaneafpemajipbefdfblo/",
  "chrome-extension://<STORE-ID>/"
]
```

Do the same with the Edge Add-ons ID if you publish there too. Until this is done the store version shows
"Gale is not installed".

## 3. Store listing tab

| Field | Value |
|---|---|
| Title | Gale Download Manager (from manifest) |
| Summary | from manifest: *Send downloads, links and videos from Chrome to Gale, the free download manager for Windows.* |
| Description | "Long description" in listing.md |
| Category | Productivity → Tools |
| Language | English |
| Store icon | `assets/chrome/icon-128.png` |
| Screenshots | `assets/chrome/screenshot-1…5.png` (1280×800) |
| Small promo tile | `assets/chrome/promo-small-440x280.png` |
| Marquee promo tile | `assets/chrome/promo-marquee-1400x560.png` |
| Official URL | https://faizzyhon.com (verify the site in Google Search Console first, or leave empty) |
| Homepage URL | https://github.com/faizzyhon/Gale |
| Support URL | https://github.com/faizzyhon/Gale/issues |
| Mature content | No |

Add this line to the top of the description so reviewers and users know the app is required:

> Requires the free Gale app for Windows: https://github.com/faizzyhon/Gale/releases

## 4. Privacy practices tab

**Single purpose**

> Gale sends the user's downloads, selected links and page media to the Gale download manager app installed on the
> same computer, so they are downloaded faster and can be paused and resumed.

**Permission justifications**

| Permission | Justification |
|---|---|
| `downloads` | To see downloads the user starts in Chrome, hand matching ones to the Gale app, and cancel the Chrome copy so the file is not downloaded twice. |
| `nativeMessaging` | To send download requests to the Gale desktop app on the same computer. This is the extension's only communication channel; it has no server. |
| `contextMenus` | Adds "Download with Gale", "Add to Gale queue", "Download all links", "Download selected links", "Download video on this page" and "Copy download address" to the right-click menu. |
| `storage` | Saves the user's extension settings (which file types to send, excluded sites, pause timer). |
| `cookies` | Reads the cookies for the single download address being sent, so files from sites the user is signed in to download correctly in Gale. Can be turned off in options. Cookies go only to the local app and are never stored. |
| `webRequest` | Observes response headers to recognise video and audio streams on the page, so the Download button can offer them. Requests are not blocked or modified. |
| `tabs` | Reads the current tab's address and title to label downloads and for "Download video on this page"; re-attaches the content script to open tabs after an update. |
| `scripting` | Runs a one-time script when the user picks "Download all links" / "Download selected links" (to collect the links) or "Copy download address" (to write to the clipboard). |
| Host permission `<all_urls>` | Downloads and media can come from any website, so the cookie lookup, media detection and the Download button must work on any site the user visits. |
| Content script on all pages | Shows the Download button when the user hovers a video or audio element (it reads only that element's source address and video height), and passes magnet links the user clicks to the Gale app. |

**Remote code:** No, I am not using remote code. All code is in the package.

**Data usage** — tick these, because the extension handles them (only to pass them to the local app):

- [x] Web history — addresses of downloads and media the user chooses to download, and of video pages on video
  sites (sent to the local app to prepare the quality menu)
- [x] Authentication information — cookies for a download's address, when "Send cookies" is on
- [x] Website content — links on the page, only when the user picks "Download all/selected links"

Leave everything else unticked. Then certify all three statements:

- [x] I do not sell or transfer user data to third parties, outside of the approved use cases
- [x] I do not use or transfer user data for purposes that are unrelated to my item's single purpose
- [x] I do not use or transfer user data to determine creditworthiness or for lending purposes

**Privacy policy URL:** https://github.com/faizzyhon/Gale/blob/main/PRIVACY.md

## 5. Distribution tab

- Visibility: Public
- Regions: All regions
- Price: Free

## 6. Account requirements (one time)

- Chrome Web Store developer account (one-time US$5 registration fee)
- Verified contact email on the account (Account tab)
- 2-step verification on the Google account

## 7. Notes for the reviewer (optional field)

> The extension is the browser companion of Gale, an open-source download manager for Windows
> (https://github.com/faizzyhon/Gale). Without the app installed the popup shows "Gale is
> not installed" and downloads continue in Chrome normally. The extension has no server and sends data only to the
> local app through native messaging. Video downloads are disabled on YouTube in this build.

---

# Microsoft Edge Add-ons

Edge accepts the same package: upload `Gale-Chrome-Web-Store-0.5.0.zip` at
https://partner.microsoft.com/dashboard/microsoftedge (free account). Use the same listing text, screenshots,
category (Productivity), privacy policy URL and permission explanations as above. Edge also assigns its own ID — add
it to `ALLOWED_ORIGINS` and `allowed_origins` as in step 2.
