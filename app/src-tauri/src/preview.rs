//! Live preview: a capture thread sends JPEG frames over a Tauri channel.
//!
//! Flow control: a frame is only sent after the frontend has acknowledged
//! the previous one (`preview_ready`), so a slow webview drops frames
//! instead of queueing them.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use obsbot_core::companion::{self, Role};
use obsbot_core::preview::{self, PreviewConfig, PreviewFormat};
use obsbot_core::virtualcam;
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::{AppHandle, Emitter, State};

#[derive(Default)]
pub struct PreviewState(Mutex<Option<Running>>);

struct Running {
    stop: Arc<AtomicBool>,
    ready: Arc<AtomicBool>,
    thread: JoinHandle<()>,
}

impl PreviewState {
    fn stop(&self) {
        if let Some(r) = self.0.lock().unwrap().take() {
            r.stop.store(true, Ordering::Relaxed);
            // The thread exits after its next frame; wait briefly so the
            // camera is free for a restart.
            for _ in 0..40 {
                if r.thread.is_finished() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(25));
            }
        }
    }
}

#[tauri::command(async)]
pub fn start_preview(
    app: AppHandle,
    state: State<PreviewState>,
    path: PathBuf,
    config: PreviewConfig,
    frames: Channel<InvokeResponseBody>,
) -> Result<PreviewFormat, String> {
    state.stop();
    let stop = Arc::new(AtomicBool::new(false));
    let ready = Arc::new(AtomicBool::new(true));
    let (started_tx, started_rx) = mpsc::channel::<Result<PreviewFormat, String>>();

    let thread = {
        let stop = stop.clone();
        let ready = ready.clone();
        std::thread::Builder::new()
            .name("preview".into())
            .spawn(move || {
                let started = started_tx.clone();
                let on_start = |fmt| {
                    let _ = started.send(Ok(fmt));
                };
                let on_frame = |jpeg: &[u8]| {
                    if ready.swap(false, Ordering::AcqRel) {
                        frames.send(InvokeResponseBody::Raw(jpeg.to_vec())).is_ok()
                    } else {
                        true
                    }
                };
                // While sharing, the share process sends the frames over a
                // socket (the camera itself is busy).
                let result = if path == virtualcam::socket_path() {
                    virtualcam::read_shared(&path, &stop, on_start, on_frame)
                        .map_err(preview::PreviewError::from)
                } else {
                    preview::run(&path, config, &stop, on_start, on_frame)
                };
                if let Err(e) = result {
                    // Before the first frame this reaches start_preview;
                    // afterwards the frontend hears about it as an event.
                    if started_tx.send(Err(e.to_string())).is_err() && !stop.load(Ordering::Relaxed)
                    {
                        let _ = app.emit("preview-stopped", e.to_string());
                    }
                }
            })
            .map_err(|e| e.to_string())?
    };

    let result = started_rx
        .recv_timeout(Duration::from_secs(5))
        .unwrap_or_else(|_| Err("timed out starting the preview".into()));
    if result.is_ok() {
        *state.0.lock().unwrap() = Some(Running {
            stop,
            ready,
            thread,
        });
    } else {
        stop.store(true, Ordering::Relaxed);
    }
    result
}

#[tauri::command]
pub fn preview_ready(state: State<PreviewState>) {
    if let Some(r) = state.0.lock().unwrap().as_ref() {
        r.ready.store(true, Ordering::Release);
    }
}

#[tauri::command(async)]
pub fn stop_preview(state: State<PreviewState>) {
    state.stop();
}

/// The virtual camera ("OBSCura Camera") and whether the camera is being
/// shared through it.
#[derive(serde::Serialize)]
pub struct ShareStatus {
    /// The virtual camera's node; `None` until v4l2loopback is loaded.
    device: Option<PathBuf>,
    /// `obsbotctl share` is feeding it.
    running: bool,
    /// Where the preview reads the shared frames.
    socket: PathBuf,
    /// Commands that set the virtual camera up (for the setup dialog).
    setup: Vec<&'static str>,
}

fn share_status_now() -> ShareStatus {
    ShareStatus {
        device: virtualcam::find(),
        running: companion::running(Role::Share).is_some(),
        socket: virtualcam::socket_path(),
        setup: virtualcam::SETUP.to_vec(),
    }
}

#[tauri::command]
pub fn share_status() -> ShareStatus {
    share_status_now()
}

/// Starts feeding the virtual camera from `camera`, in a separate process
/// that keeps running when this window closes. The preview must be stopped
/// first (the camera allows one reader); afterwards it reads the shared
/// frames from the share process. Waits until the share process has
/// registered.
#[tauri::command(async)]
pub fn share_start(
    state: State<PreviewState>,
    camera: PathBuf,
    config: PreviewConfig,
) -> Result<ShareStatus, String> {
    if virtualcam::find().is_none() {
        return Err("the virtual camera isn't set up".into());
    }
    state.stop();
    if companion::running(Role::Share).is_none() {
        let args = [
            "share".to_string(),
            "--device".into(),
            camera.display().to_string(),
            "--size".into(),
            config.height.to_string(),
            "--fps".into(),
            config.fps.to_string(),
        ];
        companion::launch_with(Role::Share, &args).map_err(|e| e.to_string())?;
    }
    for _ in 0..40 {
        if companion::running(Role::Share).is_some() {
            // Give it a moment to open the camera and announce the format.
            std::thread::sleep(Duration::from_millis(1500));
            return Ok(share_status_now());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Err("the share process didn't start".into())
}

#[tauri::command(async)]
pub fn share_stop(state: State<PreviewState>) -> ShareStatus {
    state.stop();
    companion::terminate(Role::Share);
    // Let it release the camera before the preview reopens it.
    for _ in 0..20 {
        if companion::running(Role::Share).is_none() {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    std::thread::sleep(Duration::from_millis(300));
    share_status_now()
}
