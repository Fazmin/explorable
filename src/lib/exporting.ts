import { save } from "@tauri-apps/plugin-dialog";
import { api } from "./api";
import { buildStandaloneDocument } from "../sandbox/frame";
import { currentTheme } from "./theme";
import type { ArtifactKind } from "./types";

export function slugify(title: string): string {
  return (
    title
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-+|-+$/g, "")
      .slice(0, 60) || "export"
  );
}

/** Builds the export, asks where to save it, and writes it. Returns false if cancelled. */
export async function exportArtifact(
  kind: ArtifactKind,
  title: string,
  payload: Record<string, unknown>,
  format: string,
): Promise<boolean> {
  let contents: string;
  let extension: string;
  if (kind === "interactive") {
    contents = buildStandaloneDocument(currentTheme(), title, payload as never);
    extension = "html";
  } else {
    ({ contents, extension } = await api.exportItem(kind, format, payload));
  }
  const last = extension.split(".").pop() ?? extension;
  const path = await save({
    defaultPath: `${slugify(title)}.${extension}`,
    filters: [{ name: last.toUpperCase(), extensions: [last] }],
  });
  if (!path) return false;
  await api.writeTextFile(path, contents);
  return true;
}

export function formatDate(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return "";
  const now = new Date();
  const sameDay = d.toDateString() === now.toDateString();
  if (sameDay) return d.toLocaleTimeString(undefined, { hour: "numeric", minute: "2-digit" });
  const sameYear = d.getFullYear() === now.getFullYear();
  return d.toLocaleDateString(undefined, { month: "short", day: "numeric", ...(sameYear ? {} : { year: "numeric" }) });
}
