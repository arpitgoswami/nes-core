//! Minimal ALSA playback backend, loaded at runtime.

use std::ffi::{c_char, c_int, c_long, c_void, CString};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use libloading::Library;

use super::SampleQueue;

const SND_PCM_STREAM_PLAYBACK: c_int = 0;
const SND_PCM_FORMAT_FLOAT_LE: c_int = 14;
const SND_PCM_ACCESS_RW_INTERLEAVED: c_int = 3;

type PcmOpen = unsafe extern "C" fn(*mut *mut c_void, *const c_char, c_int, c_int) -> c_int;
type PcmSetParams = unsafe extern "C" fn(*mut c_void, c_int, c_int, u32, u32, c_int, u32) -> c_int;
type PcmWriteI = unsafe extern "C" fn(*mut c_void, *const c_void, usize) -> c_long;
type PcmRecover = unsafe extern "C" fn(*mut c_void, c_int, c_int) -> c_int;
type PcmDrain = unsafe extern "C" fn(*mut c_void) -> c_int;
type PcmClose = unsafe extern "C" fn(*mut c_void) -> c_int;

struct AlsaApi {
    _library: Library,
    open: PcmOpen,
    set_params: PcmSetParams,
    write_i: PcmWriteI,
    recover: PcmRecover,
    drain: PcmDrain,
    close: PcmClose,
}

// Function pointers into a shared library are safe to use from another thread as
// long as ALSA itself serializes the individual stream (which it does).
unsafe impl Send for AlsaApi {}
unsafe impl Sync for AlsaApi {}

pub struct AlsaOutput {
    running: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

struct PcmHandle(*mut c_void);

// The PCM handle is created, used, and closed on the audio thread. The pointer
// is never aliased from the control thread, so moving it into the thread is safe.
unsafe impl Send for PcmHandle {}

impl AlsaOutput {
    pub fn start(queue: Arc<SampleQueue>, sample_rate: u32) -> Result<Self, String> {
        let api = unsafe { Self::load_api() }?;
        let api = Arc::new(api);

        let mut handle: *mut c_void = std::ptr::null_mut();
        let name = CString::new("default").map_err(|e| e.to_string())?;
        let result = unsafe { (api.open)(&mut handle, name.as_ptr(), SND_PCM_STREAM_PLAYBACK, 0) };
        if result < 0 || handle.is_null() {
            return Err(format!(
                "ALSA could not open the default playback device (error {result})"
            ));
        }

        let result = unsafe {
            (api.set_params)(
                handle,
                SND_PCM_FORMAT_FLOAT_LE,
                SND_PCM_ACCESS_RW_INTERLEAVED,
                2,
                sample_rate,
                1,
                100_000,
            )
        };
        if result < 0 {
            unsafe {
                (api.close)(handle);
            }
            return Err(format!("ALSA rejected the audio format (error {result})"));
        }

        let running = Arc::new(AtomicBool::new(true));
        let thread_running = running.clone();
        let thread_api = api.clone();
        let handle = PcmHandle(handle);
        let thread = thread::Builder::new()
            .name("nes-core-audio".into())
            .spawn(move || {
                let handle = handle;
                let mut buffer = [0.0f32; 4096];
                while thread_running.load(Ordering::Relaxed) {
                    queue.fill_interleaved(&mut buffer);
                    let frames = buffer.len() / 2;
                    let written = unsafe {
                        (thread_api.write_i)(handle.0, buffer.as_ptr() as *const c_void, frames)
                    };
                    if written < 0 {
                        let recovered =
                            unsafe { (thread_api.recover)(handle.0, written as c_int, 1) };
                        if recovered < 0 {
                            break;
                        }
                    }
                }
                unsafe {
                    (thread_api.drain)(handle.0);
                    (thread_api.close)(handle.0);
                }
            })
            .map_err(|e| e.to_string())?;

        Ok(Self {
            running,
            thread: Some(thread),
        })
    }

    unsafe fn load_api() -> Result<AlsaApi, String> {
        let library = unsafe { Library::new("libasound.so.2") }
            .or_else(|_| unsafe { Library::new("libasound.so") })
            .map_err(|e| format!("ALSA library is not available: {e}"))?;
        unsafe {
            let open = *library
                .get::<PcmOpen>(b"snd_pcm_open\0")
                .map_err(|e| e.to_string())?;
            let set_params = *library
                .get::<PcmSetParams>(b"snd_pcm_set_params\0")
                .map_err(|e| e.to_string())?;
            let write_i = *library
                .get::<PcmWriteI>(b"snd_pcm_writei\0")
                .map_err(|e| e.to_string())?;
            let recover = *library
                .get::<PcmRecover>(b"snd_pcm_recover\0")
                .map_err(|e| e.to_string())?;
            let drain = *library
                .get::<PcmDrain>(b"snd_pcm_drain\0")
                .map_err(|e| e.to_string())?;
            let close = *library
                .get::<PcmClose>(b"snd_pcm_close\0")
                .map_err(|e| e.to_string())?;
            Ok(AlsaApi {
                _library: library,
                open,
                set_params,
                write_i,
                recover,
                drain,
                close,
            })
        }
    }
}

impl Drop for AlsaOutput {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
