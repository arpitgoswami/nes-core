//! Audio output.
//!
//! Windows and macOS use cpal. Linux uses a small ALSA binding that is loaded at
//! runtime with `libloading`, which keeps the build free of audio development
//! headers; the emulator reports a clear error and runs silently if no device or
//! library is available.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

pub const SAMPLE_RATE: u32 = 44_100;

pub(crate) struct SampleQueue {
    samples: Mutex<VecDeque<f32>>,
    capacity: usize,
}

impl SampleQueue {
    fn new(sample_rate: u32) -> Self {
        Self {
            samples: Mutex::new(VecDeque::new()),
            capacity: sample_rate as usize * 2,
        }
    }

    fn push(&self, samples: &[f32], volume: f32) {
        let Ok(mut queue) = self.samples.lock() else {
            return;
        };
        if queue.len() + samples.len() * 2 > self.capacity {
            let excess = (queue.len() + samples.len() * 2).saturating_sub(self.capacity);
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
        if let Ok(mut queue) = self.samples.lock() {
            queue.clear();
        }
    }

    fn fill_interleaved(&self, output: &mut [f32]) {
        if let Ok(mut queue) = self.samples.lock() {
            for slot in output.iter_mut() {
                *slot = queue.pop_front().unwrap_or(0.0);
            }
        } else {
            output.fill(0.0);
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
