// Chrome Web Store build (`node build.mjs --store`): YouTube video downloads are switched off there,
// because the store rejects extensions that download YouTube media. The Gale app itself still supports it.
declare const __STORE__: boolean;
export const STORE_BUILD = __STORE__;

const YOUTUBE = /(^|\.)(youtube\.com|youtu\.be|youtube-nocookie\.com|googlevideo\.com)$/i;

/** True when this build must not offer media downloads for `url`. */
export function mediaBlocked(url: string | undefined, store = STORE_BUILD): boolean {
  if (!store || !url) return false;
  try {
    return YOUTUBE.test(new URL(url).hostname);
  } catch {
    return false;
  }
}
