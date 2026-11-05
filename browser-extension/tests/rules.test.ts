import assert from "node:assert/strict";
import { test } from "node:test";
import { DEFAULT_SETTINGS, decide, decideMagnet, hostMatches, isDownloadLink, videoPageKind, type AppRules, type Settings } from "../src/shared/rules";

const s = (p: Partial<Settings> = {}): Settings => ({ ...DEFAULT_SETTINGS, ...p });
const app: AppRules = { enabled: true, excludedDomains: ["corp.example"], excludedTypes: ["msi"] };
const NOW = 1_000_000;
const zip = { url: "https://cdn.example.com/files/big.zip", filename: "C:\\Users\\me\\Downloads\\big.zip", fileSize: 50 * 1024 * 1024 };

test("intercepts selected types by default", () => {
  assert.deepEqual(decide(zip, s(), app, NOW), { intercept: true, silent: false });
  assert.equal(decide({ ...zip, filename: "page.html", url: "https://x.com/page.html" }, s(), app, NOW).intercept, false);
  assert.equal(decide({ ...zip, filename: "page.html", url: "https://x.com/page.html" }, s({ mode: "all" }), app, NOW).intercept, true);
});

test("respects the user's switches and bypasses", () => {
  assert.equal(decide(zip, s({ enabled: false }), app, NOW).intercept, false);
  assert.equal(decide(zip, s({ pausedUntil: NOW + 1 }), app, NOW).intercept, false);
  assert.equal(decide(zip, s({ pausedUntil: NOW - 1 }), app, NOW).intercept, true);
  assert.equal(decide(zip, s({ bypassNext: true }), app, NOW).intercept, false);
  assert.deepEqual(decide(zip, s({ askFirst: false }), app, NOW), { intercept: true, silent: true });
});

test("app rules win", () => {
  assert.equal(decide(zip, s(), { ...app, enabled: false }, NOW).intercept, false);
  assert.equal(decide({ ...zip, url: "https://dl.corp.example/a.zip" }, s(), app, NOW).intercept, false);
  assert.equal(decide({ ...zip, filename: "setup.msi", url: "https://x.com/setup.msi" }, s(), app, NOW).intercept, false);
});

test("size threshold ignores unknown sizes", () => {
  assert.equal(decide({ ...zip, fileSize: 1024 }, s({ minSizeMB: 5 }), app, NOW).intercept, false);
  assert.equal(decide({ ...zip, fileSize: -1 }, s({ minSizeMB: 5 }), app, NOW).intercept, true);
});

test("browser-only URLs are never taken", () => {
  for (const url of ["blob:https://x.com/abc", "data:application/zip;base64,AA", "file:///C:/a.zip"]) {
    assert.equal(decide({ ...zip, url }, s({ mode: "all" }), null, NOW).intercept, false, url);
  }
});

test("domain matching includes subdomains only", () => {
  assert.ok(hostMatches("a.b.example.com", ["example.com"]));
  assert.ok(hostMatches("example.com", ["*.example.com"]));
  assert.ok(!hostMatches("notexample.com", ["example.com"]));
  assert.ok(!hostMatches("x.com", [" "]));
});

test("download link detection", () => {
  assert.ok(isDownloadLink("https://x.com/a/b.ISO?x=1", ["iso"]));
  assert.ok(!isDownloadLink("https://x.com/a/b.html", ["iso"]));
  assert.ok(!isDownloadLink("javascript:void(0)", ["iso"]));
});

test("store build blocks YouTube media only", async () => {
  const { mediaBlocked } = await import("../src/shared/store");
  assert.equal(mediaBlocked("https://www.youtube.com/watch?v=x", true), true);
  assert.equal(mediaBlocked("https://m.youtube.com/watch?v=x", true), true);
  assert.equal(mediaBlocked("https://rr1---sn.googlevideo.com/videoplayback", true), true);
  assert.equal(mediaBlocked("https://notyoutube.com/v", true), false);
  assert.equal(mediaBlocked("https://vimeo.com/1", true), false);
  assert.equal(mediaBlocked("https://www.youtube.com/watch?v=x", false), false);
});

test("torrents are caught whatever the type list says", () => {
  const t = { url: "https://tracker.example/dl.php?id=5", filename: "C:\\Users\\me\\Downloads\\ubuntu.torrent", mime: "application/x-bittorrent", fileSize: 30_000 };
  assert.equal(decide(t, s({ fileTypes: ["zip"], minSizeMB: 5 }), app, NOW).intercept, true);
  assert.equal(decide(t, s({ fileTypes: ["zip"], catchTorrents: false }), app, NOW).intercept, false);
  assert.equal(decide({ ...t, filename: "x", mime: "application/octet-stream", url: "https://x.org/a.torrent" }, s({ fileTypes: [] }), app, NOW).intercept, true);
});

test("magnet clicks follow the switches", () => {
  assert.deepEqual(decideMagnet("https://site.example/t/1", s(), app, NOW), { intercept: true, silent: false });
  assert.equal(decideMagnet("https://site.example/t/1", s({ catchTorrents: false }), app, NOW).intercept, false);
  assert.equal(decideMagnet("https://site.example/t/1", s({ enabled: false }), app, NOW).intercept, false);
  assert.equal(decideMagnet("https://a.corp.example/t/1", s(), app, NOW).intercept, false);
  assert.equal(decideMagnet("https://site.example/t/1", s(), { ...app, enabled: false }, NOW).intercept, false);
});

test("video pages are recognised", () => {
  assert.equal(videoPageKind("https://www.youtube.com/watch?v=dQw4w9WgXcQ"), "video");
  assert.equal(videoPageKind("https://m.youtube.com/shorts/abc123"), "video");
  assert.equal(videoPageKind("https://www.youtube.com/"), "site");
  assert.equal(videoPageKind("https://www.youtube.com/@channel"), "site");
  assert.equal(videoPageKind("https://youtu.be/dQw4w9WgXcQ"), "video");
  assert.equal(videoPageKind("https://vimeo.com/76979871"), "video");
  assert.equal(videoPageKind("https://x.com/user/status/123"), "video");
  assert.equal(videoPageKind("https://news.example/article"), "other");
  assert.equal(videoPageKind("not a url"), "other");
});
