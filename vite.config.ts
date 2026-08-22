import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { createReadStream, existsSync } from "node:fs";
import type { ServerResponse } from "node:http";
import { extname, join } from "node:path";
import { fileURLToPath, URL } from "node:url";
import type { Plugin } from "vite";

const host = process.env.TAURI_DEV_HOST;
const previewCoverRoot = process.env.AURORA_PREVIEW_COVER_ROOT ?? "C:\\_code\\music_backup_v5\\AlbumCovers";
const previewCovers: Readonly<Record<string, string>> = {
  "preview-viva": "Coldplay - Viva La Vida Or Death And All His Friends (2008).jpg",
  "preview-plastic-beach": "Gorillaz - Plastic Beach (2010).jpg",
  "preview-discovery": "Daft Punk - Discovery (2001).jpg",
  "preview-hurry-up": "M83 - Hurry up, We're Dreaming (2011).jpg",
  "preview-rainbows": "Radiohead - In Rainbows (2007).jpg",
  "preview-american-idiot": "Green Day - American Idiot (2004).jpg",
  "preview-drive": "Cliff Martinez - Drive (2011).jpg",
  "preview-outrun": "Kavinsky - OutRun (2013).jpg",
  "preview-chart-crowd": "The Crowd - You'll Never Walk Alone (1985).jpg",
  "preview-chart-kayleigh": "Marillion - Misplaced Childhood (1985).jpg",
  "preview-chart-19": "Paul Hardcastle - Paul Hardcastle (1985).jpg",
  "preview-chart-suddenly": "Billy Ocean - Suddenly (1984).jpg",
  "preview-chart-obsession": "Animotion - Animotion (1984).jpg",
  "preview-chart-view": "John Barry - A View To A Kill (1985).jpg",
  "preview-chart-fields": "Gary Moore - Run For Cover (1985).jpg",
  "preview-chart-word": "Scritti Politti - Cupid & Psyche 85 (1985).jpg",
  "preview-chart-crazy": "Madonna - Like a Virgin (1984).jpg",
  "preview-chart-history": "Mai Tai - Mai Tai (1985).jpg",
  "preview-score-rocky": "Various Artists - Rocky IV (1985).jpg",
  "preview-score-miami": "Various Artists - Miami Vice (1985).jpg",
  "preview-score-back-future": "Various Artists - Back to the Future (1985).jpg",
  "preview-score-american-flyers": "Various Artists - American Flyers (1985).jpg",
  "preview-score-magnum": "Magnum - On a Storyteller's Night (1985).jpg",
};

function previewCoverPlugin(): Plugin {
  return {
    name: "aurora-preview-covers",
    configureServer(server) {
      server.middlewares.use((request, response, next) => {
        const prefix = "/__aurora-preview-cover/";
        if (!request.url?.startsWith(prefix)) {
          next();
          return;
        }
        const albumId = decodeURIComponent(request.url.slice(prefix.length).split("?")[0]);
        const filename = previewCovers[albumId];
        const coverPath = filename ? join(previewCoverRoot, filename) : null;
        if (!coverPath || !existsSync(coverPath)) {
          response.statusCode = 404;
          response.end();
          return;
        }
        response.setHeader("Content-Type", extname(coverPath).toLowerCase() === ".png" ? "image/png" : "image/jpeg");
        createReadStream(coverPath).pipe(response as ServerResponse);
      });
    },
  };
}

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [react(), previewCoverPlugin()],
  resolve: {
    alias: {
      "@": fileURLToPath(new URL("./src", import.meta.url)),
    },
  },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1430,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1431,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
  test: {
    environment: "jsdom",
    setupFiles: "./src/test/setup.ts",
  },
}));
