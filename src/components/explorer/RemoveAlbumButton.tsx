import { FolderOutput } from "lucide-react";

export function RemoveAlbumButton({ onRequest, disabled }: {
  onRequest: () => void;
  disabled?: boolean;
}) {
  return <div className="remove-album-action">
    <button type="button" className="deep-explorer-move-inbox" disabled={disabled} onClick={onRequest}>
      <FolderOutput aria-hidden="true" />Remove Album
    </button>
  </div>;
}
