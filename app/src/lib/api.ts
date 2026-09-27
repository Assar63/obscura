// Typed wrappers around the Tauri commands in src-tauri/src/commands.rs.
import { Channel, invoke } from "@tauri-apps/api/core";

export type ChoiceOption = { value: number; label: string };

export type FeatureKind =
  | { type: "toggle" }
  | { type: "choice"; options: ChoiceOption[] }
  | {
      type: "range";
      min: number;
      max: number;
      step: number;
      default: number | null;
      scale: number;
      unit: string | null;
    }
  | { type: "action" };

export type FeatureState = {
  id: string;
  group: string;
  label: string;
  kind: FeatureKind;
  supported: boolean;
  active: boolean;
  value: number | null;
  reason: string | null;
};

export type CameraInfo = {
  path: string;
  name: string;
  vendor_id: number;
  product_id: number;
  manufacturer: string | null;
  product: string | null;
  serial: string | null;
  usb_version: string | null;
  usb_path: string;
  is_obsbot: boolean;
};

export type Snapshot = {
  info: CameraInfo;
  profile_id: string;
  profile_name: string;
  gimbal_velocity: boolean;
  firmware: { version: string; serial: string | null } | null;
  features: FeatureState[];
};

export type PreviewConfig = { width: number; height: number; fps: number };
export type PreviewFormat = PreviewConfig;

export const api = {
  appOptions: () => invoke<{ include_all: boolean }>("app_options"),
  featureCatalog: () => invoke<FeatureState[]>("feature_catalog"),
  listCameras: () => invoke<CameraInfo[]>("list_cameras"),
  openCamera: (path: string) => invoke<Snapshot>("open_camera", { path }),
  getFeatures: () => invoke<FeatureState[]>("get_features"),
  setFeature: (id: string, value: number) =>
    invoke<FeatureState[]>("set_feature", { id, value }),
  /** Starts streaming JPEG frames to `onFrame`. Call `previewReady` after
   * each frame is drawn to receive the next one. */
  startPreview: (path: string, config: PreviewConfig, onFrame: (jpeg: ArrayBuffer) => void) => {
    const frames = new Channel<ArrayBuffer | number[]>();
    frames.onmessage = (m) => onFrame(m instanceof ArrayBuffer ? m : new Uint8Array(m).buffer);
    return invoke<PreviewFormat>("start_preview", { path, config, frames });
  },
  getAutostart: () => invoke<boolean>("get_autostart"),
  setAutostart: (enabled: boolean) => invoke<boolean>("set_autostart", { enabled }),
  gimbalMove: (right: number, up: number) => invoke<void>("gimbal_move", { right, up }),
  previewReady: () => invoke<void>("preview_ready"),
  stopPreview: () => invoke<void>("stop_preview"),
};

export function formatValue(kind: FeatureKind, value: number): string {
  if (kind.type !== "range") return String(value);
  if (kind.scale <= 1) return `${value}`;
  const decimals = Math.min(2, Math.ceil(Math.log10(kind.scale)));
  return (value / kind.scale).toFixed(decimals);
}
