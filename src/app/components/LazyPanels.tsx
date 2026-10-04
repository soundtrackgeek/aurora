import { lazy, Suspense, type ComponentProps } from "react";
import type { AlbumMoveOperation as AlbumMoveComponent } from "../../components/explorer/AlbumMoveOperation";
import type { SettingsDialog as SettingsComponent } from "../../components/SettingsDialog";
import type { TagEditor as TagEditorComponent } from "../../components/TagEditor";

const LazyAlbumMove = lazy(async () => ({ default: (await import("../../components/explorer/AlbumMoveOperation")).AlbumMoveOperation }));
const LazySettings = lazy(async () => ({ default: (await import("../../components/SettingsDialog")).SettingsDialog }));
const LazyTagEditor = lazy(async () => ({ default: (await import("../../components/TagEditor")).TagEditor }));

function DialogLoading({ label }: { label: string }) {
  return <div className="modal-backdrop"><div className="settings-loading" role="status">{label}</div></div>;
}

export function AlbumMoveOperation(props: ComponentProps<typeof AlbumMoveComponent>) {
  return <Suspense fallback={<DialogLoading label="Opening album operation…" />}><LazyAlbumMove {...props} /></Suspense>;
}

export function SettingsDialog(props: ComponentProps<typeof SettingsComponent>) {
  return <Suspense fallback={<DialogLoading label="Opening settings…" />}><LazySettings {...props} /></Suspense>;
}

export function TagEditor(props: ComponentProps<typeof TagEditorComponent>) {
  return <Suspense fallback={<div className="settings-loading" role="status">Opening tag editor…</div>}><LazyTagEditor {...props} /></Suspense>;
}
