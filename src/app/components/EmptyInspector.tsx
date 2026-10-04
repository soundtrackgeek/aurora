import { Disc3 } from "lucide-react";

export function EmptyInspector() {
  return (
    <div className="inspector-empty">
      <Disc3 aria-hidden="true" />
      <h2>Select a track</h2>
      <p>Select a song, then press play or double-click its library row.</p>
    </div>
  );
}
