import { useEffect, useRef, type ReactNode } from "react";

export function ContextTagDialog({ children, onClose }: { children: ReactNode; onClose: () => void }) {
  const dialog = useRef<HTMLDialogElement>(null);
  useEffect(() => { dialog.current?.showModal(); }, []);
  return <dialog ref={dialog} className="context-tag-dialog" aria-label="Context Tags" onCancel={event => { event.preventDefault(); onClose(); }}>
    <button type="button" aria-label="Close Tags" onClick={onClose}>Close</button>
    {children}
  </dialog>;
}
