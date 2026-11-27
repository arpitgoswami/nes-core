//! Audio output.
//!
//! Windows and macOS use cpal. Linux uses a small ALSA binding that is loaded at
//! runtime with `libloading`, which keeps the build free of audio development
//! headers; the emulator reports a clear error and runs silently if no device or
//! library is available.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

pub const SAMPLE_RATE: u32 = 44_100;

struct QueueState {
    samples: VecDeque<f32>,
    /// Last sample played, held and faded out when the queue runs dry so an
    /// underrun makes a soft fade instead of a click.
    last: f32,
}

pub(crate) struct SampleQueue {
    state: Mutex<QueueState>,
    capacity: usize,
    sample_rate: u32,
}

impl SampleQueue {
    fn new(sample_rate: u32) -> Self {
        Self {
            state: Mutex::new(QueueState {
                samples: VecDeque::new(),
                last: 0.0,
            }),
            capacity: sample_rate as usize * 2,
            sample_rate,
        }
    }

    /// Seconds of audio waiting to be played.
    fn buffered_seconds(&self) -> f32 {
        let len = self
            .state
            .lock()
            .map(|state| state.samples.len())
            .unwrap_or(0);
        len as f32 / 2.0 / self.sample_rate as f32
    }

    fn push(&self, samples: &[f32], volume: f32) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        let queue = &mut state.samples;
        // Keep at most about half a second queued. If the game outruns the
        // device, drop whole stereo frames from the oldest end.
        let incoming = samples.len() * 2;
        if queue.len() + incoming > self.capacity {
            let excess = (queue.len() + incoming - self.capacity).div_ceil(2) * 2;
            let len = queue.len();
            queue.drain(..excess.min(len));
        }
        for sample in samples {
            let value = (sample * volume).clamp(-1.0, 1.0);
            queue.push_back(value);
            queue.push_back(value);
        }
    }

    fn clear(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.samples.clear();
            state.last = 0.0;
        }
    }

    fn fill_interleaved(&self, output: &mut [f32]) {
        let Ok(mut state) = self.state.lock() else {
            output.fill(0.0);
            return;
        };
        for slot in output.iter_mut() {
            *slot = match state.samples.pop_front() {
                Some(value) => {
                    state.last = value;
                    value
                }
                None => {
                    state.last *= 0.995;
                    state.last
                }
            };
        }
    }
}

#[cfg(target_os = "linux")]
mod alsa;
#[cfg(not(target_os = "linux"))]
mod cpal_backend;

pub struct AudioOutput {
    #[cfg(target_os = "linux")]
    _backend: Option<alsa::AlsaOutput>,
    #[cfg(not(target_os = "linux"))]
    _backend: Option<cpal_backend::CpalOutput>,
    queue: Arc<SampleQueue>,
    pub error: Option<String>,
}

impl AudioOutput {
    pub fn new(sample_rate: u32) -> Self {
        let queue = Arc::new(SampleQueue::new(sample_rate));
        match Self::start_backend(queue.clone(), sample_rate) {
            Ok(backend) => Self {
                _backend: backend,
                queue,
                error: None,
            },
            Err(error) => Self {
                _backend: None,
                queue,
                error: Some(error),
            },
        }
    }

    pub fn push_mono(&self, samples: &[f32], volume: f32) {
        if samples.is_empty() {
            return;
        }
        self.queue.push(samples, volume);
    }

    pub fn clear(&self) {
        self.queue.clear();
    }

    /// Seconds of audio queued for playback.
    pub fn buffered_seconds(&self) -> f32 {
        self.queue.buffered_seconds()
    }

    pub fn is_active(&self) -> bool {
        self._backend.is_some()
    }

    #[cfg(target_os = "linux")]
    fn start_backend(
        queue: Arc<SampleQueue>,
        sample_rate: u32,
    ) -> Result<Option<alsa::AlsaOutput>, String> {
        alsa::AlsaOutput::start(queue, sample_rate).map(Some)
    }

    #[cfg(not(target_os = "linux"))]
    fn start_backend(
        queue: Arc<SampleQueue>,
        sample_rate: u32,
    ) -> Result<Option<cpal_backend::CpalOutput>, String> {
        cpal_backend::CpalOutput::start(queue, sample_rate).map(Some)
    }
}
