import type { AppSettings, Category } from "../types";

/** Same rule as gale-core's categorize(): extension list match, else "Other". */
export function categorize(categories: Category[], fileName: string): Category | undefined {
  const ext = fileName.includes(".") ? fileName.split(".").pop()!.toLowerCase() : "";
  return categories.find((c) => ext && c.extensions.split(/\s+/).includes(ext)) ?? categories.find((c) => c.name === "Other");
}

export const join = (dir: string, sub: string) => `${dir.replace(/[\\/]+$/, "")}${dir.includes("\\") ? "\\" : "/"}${sub}`;

/** Where a new download of this category starts: the folder remembered for it, else its category folder (when
 *  that setting is on), else the default download folder. */
export function folderFor(s: Pick<AppSettings, "downloadDir" | "categoryFolders" | "folders">, cat: Category | undefined): string {
  const remembered = cat ? s.folders?.[cat.name] : undefined;
  if (remembered) return remembered;
  return s.categoryFolders && cat ? join(s.downloadDir, cat.subfolder) : s.downloadDir;
}
