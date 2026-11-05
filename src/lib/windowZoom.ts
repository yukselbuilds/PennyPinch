import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";

export type WindowScale = 1 | 1.25 | 1.5;
export function createWindowZoomClient(transport: { isNative: () => boolean; setZoom: (scale: number) => Promise<void> }) {
  return {
    available: () => transport.isNative(),
    async set(scale: WindowScale): Promise<boolean> {
      try {
        if (![1, 1.25, 1.5].includes(scale) || !transport.isNative()) return false;
        await transport.setZoom(scale);
        return true;
      } catch { return false; }
    },
  };
}
export const windowZoomClient = createWindowZoomClient({ isNative: () => isTauri(), setZoom: scale => getCurrentWebview().setZoom(scale) });
