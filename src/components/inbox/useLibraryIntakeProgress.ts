import { useCallback, useEffect, useState } from "react";
import {
  listenLibraryIntakeProgress,
  type LibraryIntakeProgress,
} from "../../ingest";

export function useLibraryIntakeProgress(planId?: string) {
  const [progress, setProgress] = useState<LibraryIntakeProgress | null>(null);
  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void listenLibraryIntakeProgress((next) => {
      if (!disposed && (!planId || next.planId === planId)) setProgress(next);
    }).then((nextUnlisten) => {
      if (disposed) nextUnlisten();
      else unlisten = nextUnlisten;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [planId]);
  const reset = useCallback(() => setProgress(null), []);
  return { progress, reset };
}
