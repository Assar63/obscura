// Live preview state: requested format, the running stream, and drawing.
import { listen } from "@tauri-apps/api/event";
import { api, type PreviewFormat } from "./api";

export type Resolution = 1080 | 720;
export type Fps = 30 | 60;

const SIZES: Record<Resolution, [number, number]> = { 1080: [1920, 1080], 720: [1280, 720] };

const MIRROR_KEY = "obscura.preview.mirror";

function loadMirror(): boolean {
  try {
    return localStorage.getItem(MIRROR_KEY) === "1";
  } catch {
    return false;
  }
}

class PreviewStore {
  resolution = $state<Resolution>(1080);
  fps = $state<Fps>(30);
  /** Show the preview mirrored, like a mirror (the preview only: other
   * apps still get the camera's picture). Remembered between runs. */
  mirror = $state(loadMirror());
  /** Format the camera agreed to while streaming. */
  format = $state<PreviewFormat | null>(null);
  error = $state<string | null>(null);
  starting = $state(false);

  #canvas: HTMLCanvasElement | null = null;
  #decoding = false;
  #generation = 0;
  /** Start/stop calls run strictly in order; otherwise a late stop could
   * kill a stream that was started after it. */
  #queue: Promise<unknown> = Promise.resolve();

  #enqueue(op: () => Promise<void>) {
    this.#queue = this.#queue.then(op, op);
    return this.#queue;
  }

  constructor() {
    void listen<string>("preview-stopped", (e) => {
      this.format = null;
      this.error = e.payload;
    });
  }

  toggleMirror() {
    this.mirror = !this.mirror;
    try {
      localStorage.setItem(MIRROR_KEY, this.mirror ? "1" : "0");
    } catch {
      // Not remembered, but still applied.
    }
  }

  attach(canvas: HTMLCanvasElement | null) {
    this.#canvas = canvas;
  }

  start(path: string) {
    const gen = ++this.#generation;
    const [width, height] = SIZES[this.resolution];
    const fps = this.fps;
    this.starting = true;
    this.error = null;
    return this.#enqueue(async () => {
      if (gen !== this.#generation) return;
      try {
        const fmt = await api.startPreview(path, { width, height, fps }, (jpeg) => {
          if (gen === this.#generation) void this.#draw(jpeg);
        });
        if (gen === this.#generation) this.format = fmt;
        else await api.stopPreview();
      } catch (e) {
        if (gen === this.#generation) {
          this.format = null;
          this.error = String(e);
        }
      } finally {
        if (gen === this.#generation) this.starting = false;
      }
    });
  }

  stop() {
    this.#generation++;
    this.format = null;
    this.starting = false;
    return this.#enqueue(() => api.stopPreview());
  }

  async #draw(jpeg: ArrayBuffer) {
    const canvas = this.#canvas;
    if (this.#decoding || !canvas) {
      void api.previewReady();
      return;
    }
    this.#decoding = true;
    try {
      const bmp = await createImageBitmap(new Blob([jpeg], { type: "image/jpeg" }));
      if (canvas.width !== bmp.width || canvas.height !== bmp.height) {
        canvas.width = bmp.width;
        canvas.height = bmp.height;
      }
      canvas.getContext("2d")?.drawImage(bmp, 0, 0);
      bmp.close();
    } catch {
      // Corrupt frame; skip it.
    } finally {
      this.#decoding = false;
      void api.previewReady();
    }
  }
}

export const preview = new PreviewStore();
