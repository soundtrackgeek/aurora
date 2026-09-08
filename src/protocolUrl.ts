import { convertFileSrc } from "@tauri-apps/api/core";

export function protocolUrl(protocol: string, route: string, id: string): string {
  const base = convertFileSrc(id, protocol);
  const slash = base.indexOf("/", base.indexOf("://") + 3);
  return `${base.slice(0, slash)}/${route}${base.slice(slash)}`;
}
