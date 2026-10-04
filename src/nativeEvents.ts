import { listen } from "@tauri-apps/api/event";
import { isTauriRuntime } from "./library";

/** Register before loading initial state; release even a late async registration. */
export function subscribeNativeEvent<T>(
  name: string,
  receive: (payload: T) => void,
  ready?: () => void,
): () => void {
  let disposed = false;
  let unlisten: (() => void) | undefined;
  if (!isTauriRuntime()) {
    ready?.();
    return () => { disposed = true; };
  }
  void listen<T>(name, (event) => {
    if (!disposed) receive(event.payload);
  }).then((release) => {
    if (disposed) release();
    else {
      unlisten = release;
      ready?.();
    }
  }).catch((error: unknown) => {
    console.warn(`Aurora could not subscribe to ${name}`, error);
    if (!disposed) ready?.();
  });
  return () => { disposed = true; unlisten?.(); };
}
