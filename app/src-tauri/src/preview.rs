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

use obsbot_core::preview::{self, PreviewConfig, PreviewFormat};
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
                let result = preview::run(
                    &path,
                    config,
                    &stop,
                    |fmt| {
                        let _ = started.send(Ok(fmt));
                    },
                    |jpeg| {
                        if ready.swap(false, Ordering::AcqRel) {
                            frames.send(InvokeResponseBody::Raw(jpeg.to_vec())).is_ok()
                        } else {
                            true
                        }
                    },
                );
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
