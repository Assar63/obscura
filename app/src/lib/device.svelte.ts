// Reactive app state: cameras, the open device and its features.
import { api, type CameraInfo, type FeatureState } from "./api";

const RANGE_DEBOUNCE_MS = 60;
/** OBSBOT Center polls the camera's status every ~2 s; do the same so
 * changes made on the camera (gestures, sleep) show up. */
const POLL_MS = 2000;

class DeviceStore {
  cameras = $state<CameraInfo[]>([]);
  current = $state<CameraInfo | null>(null);
  profileName = $state<string>("");
  /** Joystick drives the gimbal by velocity rather than absolute nudges. */
  gimbalVelocity = $state(false);
  features = $state<Record<string, FeatureState>>({});
  error = $state<string | null>(null);

  #timers = new Map<string, ReturnType<typeof setTimeout>>();
  #inflight = 0;
  #polling = false;
  #includeAll = false;
  #catalog: FeatureState[] = [];

  /** Writes queued or in flight; polls are skipped meanwhile so a stale
   * read can't snap a control back while it's being changed. */
  get #busy() {
    return this.#inflight > 0 || this.#timers.size > 0;
  }

  #apply(list: FeatureState[]) {
    const next: Record<string, FeatureState> = {};
    for (const f of list) next[f.id] = f;
    this.features = next;
  }

  async init() {
    this.#catalog = await api.featureCatalog();
    this.#apply(this.#catalog);
    this.#includeAll = (await api.appOptions()).include_all;
    await this.#autoOpen();
    setInterval(() => void this.#poll(), POLL_MS);
  }

  /** Opens the first OBSBOT camera (any camera with OBSBOT_ALL_CAMERAS=1). */
  async #autoOpen() {
    await this.refreshCameras();
    const first = this.cameras.find((c) => c.is_obsbot || this.#includeAll);
    if (first) await this.open(first.path);
  }

  async #poll() {
    if (this.#busy || this.#polling) return;
    this.#polling = true;
    try {
      if (!this.current) {
        await this.#autoOpen();
        return;
      }
      const list = await api.getFeatures();
      if (!this.#busy) this.#apply(list);
    } catch {
      // The camera went away (unplugged or rebooting); wait for it.
      this.current = null;
      this.#apply(this.#catalog);
      this.error = "Camera disconnected. Waiting for it to come back…";
      await this.refreshCameras();
    } finally {
      this.#polling = false;
    }
  }

  async refreshCameras() {
    try {
      this.cameras = await api.listCameras();
    } catch (e) {
      this.error = String(e);
    }
  }

  async open(path: string) {
    try {
      const snap = await api.openCamera(path);
      this.current = snap.info;
      this.profileName = snap.profile_name;
      this.gimbalVelocity = snap.gimbal_velocity;
      this.#apply(snap.features);
      this.error = null;
      await this.refreshCameras();
    } catch (e) {
      this.error = String(e);
    }
  }

  async reload() {
    if (!this.current) return;
    try {
      this.#apply(await api.getFeatures());
    } catch (e) {
      this.error = String(e);
    }
  }

  /** Optimistically updates the UI, then writes. Range writes are debounced
   * so dragging a slider doesn't flood the camera. */
  set(id: string, value: number) {
    const f = this.features[id];
    if (!f?.supported) return;
    f.value = value;
    const write = async () => {
      this.#timers.delete(id);
      this.#inflight++;
      try {
        const list = await api.setFeature(id, value);
        if (this.#inflight === 1 && this.#timers.size === 0) this.#apply(list);
        this.error = null;
      } catch (e) {
        this.error = String(e);
        await this.reload();
      } finally {
        this.#inflight--;
      }
    };
    if (f.kind.type === "range") {
      clearTimeout(this.#timers.get(id));
      this.#timers.set(id, setTimeout(write, RANGE_DEBOUNCE_MS));
    } else {
      void write();
    }
  }

  /** Gimbal velocity from the joystick; (0, 0) stops. Errors surface in
   * the banner but don't interrupt the stream of updates. */
  async gimbalMove(right: number, up: number) {
    try {
      await api.gimbalMove(right, up);
    } catch (e) {
      this.error = String(e);
    }
  }

  /** Moves a range feature by `fraction` of its span (joystick input). */
  nudge(id: string, fraction: number) {
    const f = this.features[id];
    if (!f?.supported || f.kind.type !== "range" || fraction === 0) return;
    const { min, max, step } = f.kind;
    const delta = Math.round(((max - min) * fraction) / step) * step || Math.sign(fraction) * step;
    this.set(id, Math.min(max, Math.max(min, (f.value ?? 0) + delta)));
  }

  /** Sets range features back to the defaults the device reports. */
  resetToDefault(ids: string[]) {
    for (const id of ids) {
      const f = this.features[id];
      if (f?.supported && f.kind.type === "range" && f.kind.default !== null) {
        this.set(id, f.kind.default);
      }
    }
  }

  /** Feature value as boolean for toggles. */
  on(id: string): boolean {
    return (this.features[id]?.value ?? 0) !== 0;
  }

  supported(id: string): boolean {
    return this.features[id]?.supported ?? false;
  }
}

export const device = new DeviceStore();
