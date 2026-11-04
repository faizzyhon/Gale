import type { Category, Download, DownloadStatus } from "../types";

export type ListFilter =
  | { kind: "all" }
  | { kind: "status"; statuses: DownloadStatus[] }
  | { kind: "category"; category: string };

export interface NavItem {
  id: string;
  label: string;
  filter?: ListFilter;
}

export type View =
  | { page: "list"; nav: NavItem }
  | { page: "dashboard" }
  | { page: "history" }
  | { page: "settings" }
  | { page: "queues" }
  | { page: "grabber" };

export const STATUS_NAV: NavItem[] = [
  { id: "all", label: "All Downloads", filter: { kind: "all" } },
  { id: "progress", label: "In Progress", filter: { kind: "status", statuses: ["probing", "downloading"] } },
  { id: "completed", label: "Completed", filter: { kind: "status", statuses: ["completed"] } },
  { id: "queued", label: "Queued", filter: { kind: "status", statuses: ["queued"] } },
  { id: "scheduled", label: "Scheduled", filter: { kind: "status", statuses: ["scheduled"] } },
  { id: "paused", label: "Paused", filter: { kind: "status", statuses: ["paused"] } },
  { id: "failed", label: "Failed", filter: { kind: "status", statuses: ["failed", "cancelled"] } },
];

// Sidebar label → built-in category name in the database.
export const CATEGORY_NAV: NavItem[] = [
  { id: "cat-video", label: "Video Downloads", filter: { kind: "category", category: "Video" } },
  { id: "cat-audio", label: "Music", filter: { kind: "category", category: "Audio" } },
  { id: "cat-docs", label: "Documents", filter: { kind: "category", category: "Documents" } },
  { id: "cat-images", label: "Images", filter: { kind: "category", category: "Images" } },
  { id: "cat-arch", label: "Compressed Files", filter: { kind: "category", category: "Archives" } },
  { id: "cat-soft", label: "Applications", filter: { kind: "category", category: "Software" } },
  { id: "cat-other", label: "Other Files", filter: { kind: "category", category: "Other" } },
];

export function matches(d: Download, f: ListFilter, categories: Category[], query: string): boolean {
  if (query) {
    const q = query.toLowerCase();
    if (!d.fileName.toLowerCase().includes(q) && !d.url.toLowerCase().includes(q)) return false;
  }
  switch (f.kind) {
    case "all":
      return true;
    case "status":
      return f.statuses.includes(d.status);
    case "category": {
      const cat = categories.find((c) => c.id === d.categoryId);
      return (cat?.name ?? "Other") === f.category;
    }
  }
}
