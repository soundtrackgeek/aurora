import { FolderOutput } from "lucide-react";

export function RemoveAlbumButton({ onRequest, disabled, albumCount = 1 }: {
  onRequest: () => void;
  disabled?: boolean;
  /** More than one when the open album is part of a multi-album selection. */
  albumCount?: number;
}) {
  return <div className="remove-album-action">
    <button type="button" className="deep-explorer-move-inbox" disabled={disabled} onClick={onRequest}>
      <FolderOutput aria-hidden="true" />{albumCount > 1 ? `Remove ${albumCount} Albums` : "Remove Album"}
    </button>
  </div>;
}
