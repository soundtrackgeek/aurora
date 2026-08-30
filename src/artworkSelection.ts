import { invoke } from "@tauri-apps/api/core";
import { isTauriRuntime } from "./library";
import type { TagEditorTarget } from "./tags";

export interface SelectedArtwork {
  token: string;
  previewUrl: string;
  fileName: string;
}

export type AlbumCoverPickerRequest =
  | { source: "library"; target: TagEditorTarget }
  | { source: "inbox"; albumPath: string };

export async function selectAlbumCoverImage(
  request: AlbumCoverPickerRequest,
): Promise<SelectedArtwork | null> {
  if (!isTauriRuntime()) {
    return {
      token: "preview-selected-cover",
      previewUrl: "/__aurora-preview-cover/preview-freak?size=256",
      fileName: "selected-cover.jpg",
    };
  }
  return invoke<SelectedArtwork | null>("select_album_cover_image", { request });
}
