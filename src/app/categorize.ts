import type { Category } from "../types";

/** Same rule as gale-core's categorize(): extension list match, else "Other". */
export function categorize(categories: Category[], fileName: string): Category | undefined {
  const ext = fileName.includes(".") ? fileName.split(".").pop()!.toLowerCase() : "";
  return categories.find((c) => ext && c.extensions.split(/\s+/).includes(ext)) ?? categories.find((c) => c.name === "Other");
}
