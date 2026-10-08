import { invoke } from "@tauri-apps/api/core";
import { isTauriRuntime } from "./library";
import type { ExplorerFilters, ExplorerView } from "./components/explorer/DeepExplorer";
export interface SavedView { id: number; name: string; view: ExplorerView; filters: ExplorerFilters; }
const key = "aurora:preview-saved-views:v1";
export async function listSavedViews(): Promise<SavedView[]> {
  if (isTauriRuntime()) return invoke("list_saved_views");
  try { return JSON.parse(localStorage.getItem(key) ?? "[]") as SavedView[]; } catch { return []; }
}
export async function saveExplorerView(input: Omit<SavedView,"id"> & { id: number | null }): Promise<SavedView> {
  if (isTauriRuntime()) return invoke("save_explorer_view", { input });
  const views = await listSavedViews();
  const saved = { ...input, id: input.id ?? Math.max(0,...views.map(v => v.id)) + 1 };
  localStorage.setItem(key,JSON.stringify([...views.filter(v => v.id !== saved.id),saved]));
  return saved;
}
export async function deleteSavedView(id: number): Promise<void> {
  if (isTauriRuntime()) return invoke("delete_saved_view", { id });
  localStorage.setItem(key,JSON.stringify((await listSavedViews()).filter(v => v.id !== id)));
}
