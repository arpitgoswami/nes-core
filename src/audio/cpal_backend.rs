//! cpal playback backend for Windows and macOS.

use std::sync::Arc;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use super::SampleQueue;

pub struct CpalOutput {
    _stream: cpal::Stream,
}

impl CpalOutput {
    pub fn start(queue: Arc<SampleQueue>, sample_rate: u32) -> Result<Self, String> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| "no default audio output device".to_string())?;
        let config = cpal::StreamConfig {
            channels: 2,
            sample_rate,
            buffer_size: cpal::BufferSize::Default,
        };

        let stream = device
            .build_output_stream::<f32, _, _>(
                config,
                move |output: &mut [f32], _info: &cpal::OutputCallbackInfo| {
                    queue.fill_interleaved(output);
                },
                |error| eprintln!("audio stream error: {error}"),
                None,
            )
            .map_err(|e| format!("could not open the audio stream: {e}"))?;
        stream
            .play()
            .map_err(|e| format!("could not start audio playback: {e}"))?;
        Ok(Self { _stream: stream })
    }
}
