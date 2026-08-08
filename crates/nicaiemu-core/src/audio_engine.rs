//! Audio engine: decodes guest WAV/MP3 data, mixes stereo PCM at a fixed
//! output rate, and reports deterministic playback diagnostics.

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Output sample rate shared by every frontend (44.1 kHz stereo).
pub const AUDIO_SAMPLE_RATE: u32 = 44_100;

/// Maximum buffered stereo samples (~10 seconds) before the oldest are dropped.
const MAX_QUEUED_SAMPLES: usize = AUDIO_SAMPLE_RATE as usize * 2 * 10;

/// Deterministic audio playback evidence for headless baselines.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AudioDiagnostics {
    pub sample_rate: u32,
    pub channels: u16,
    pub volume: u32,
    pub submitted_bytes: u64,
    pub decoded_frames: u64,
    pub nonzero_samples: u64,
    pub rejected_writes: u64,
    pub underflow_frames: u64,
    pub max_buffered_frames: u64,
    pub pcm_crc32: u32,
    pub rms_amplitude: f32,
}

/// Guest audio playback state and output queue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioEngine {
    volume: u32,
    playing: bool,
    paused: bool,
    samples: VecDeque<i16>,
    submitted_bytes: u64,
    decoded_frames: u64,
    nonzero_samples: u64,
    rejected_writes: u64,
    underflow_frames: u64,
    max_buffered_frames: u64,
}

impl Default for AudioEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioEngine {
    pub fn new() -> Self {
        Self {
            volume: 100,
            playing: false,
            paused: false,
            samples: VecDeque::new(),
            submitted_bytes: 0,
            decoded_frames: 0,
            nonzero_samples: 0,
            rejected_writes: 0,
            underflow_frames: 0,
            max_buffered_frames: 0,
        }
    }

    /// Set the playback volume, clamped to 0-100.
    pub fn set_volume(&mut self, volume: u32) {
        self.volume = volume.min(100);
    }

    pub fn volume(&self) -> u32 {
        self.volume
    }

    /// Decode WAV/MP3/MIDI bytes and queue the resulting stereo PCM.
    pub fn play_bytes(&mut self, data: &[u8]) -> Result<()> {
        self.play_bytes_repeats(data, 0)
    }

    /// Decode audio bytes and queue the resulting stereo PCM.
    ///
    /// `repeats` is the number of additional loops after the first play.
    pub fn play_bytes_repeats(&mut self, data: &[u8], repeats: u32) -> Result<()> {
        self.submitted_bytes = self.submitted_bytes.saturating_add(data.len() as u64);
        let decoded = if is_wav(data) {
            let (samples, channels, rate) = decode_wav(data)?;
            resample_to_stereo(&samples, channels as usize, rate, AUDIO_SAMPLE_RATE)
        } else if is_mp3(data) {
            decode_mp3(data)?
        } else if let Some((payload_type, payload)) = audio_resource_payload(data) {
            match payload_type {
                0x000A => decode_midi(payload)?,
                _ if is_wav(payload) => {
                    let (samples, channels, rate) = decode_wav(payload)?;
                    resample_to_stereo(&samples, channels as usize, rate, AUDIO_SAMPLE_RATE)
                }
                _ if is_mp3(payload) => decode_mp3(payload)?,
                _ => {
                    self.rejected_writes += 1;
                    bail!("unknown audio resource type {payload_type:#06x}");
                }
            }
        } else {
            self.rejected_writes += 1;
            bail!("unrecognized audio data ({} bytes)", data.len());
        };

        let frames = decoded.len() / 2;
        if frames == 0 {
            self.rejected_writes += 1;
            bail!("audio data decoded to no samples");
        }
        self.nonzero_samples = self
            .nonzero_samples
            .saturating_add(decoded.iter().filter(|sample| **sample != 0).count() as u64);
        let loops = repeats.saturating_add(1).max(1) as usize;
        self.decoded_frames = self
            .decoded_frames
            .saturating_add(frames as u64 * loops as u64);
        for _ in 0..loops {
            self.samples.extend(decoded.clone());
        }
        let overflow = self.samples.len().saturating_sub(MAX_QUEUED_SAMPLES);
        if overflow > 0 {
            self.samples.drain(..overflow);
        }
        self.max_buffered_frames = self.max_buffered_frames.max(self.samples.len() as u64 / 2);
        self.playing = true;
        self.paused = false;
        Ok(())
    }

    pub fn pause(&mut self) {
        self.paused = true;
    }

    pub fn resume(&mut self) {
        self.paused = false;
    }

    pub fn stop(&mut self) {
        self.samples.clear();
        self.playing = false;
        self.paused = false;
    }

    /// Playback state: 0 stopped, 1 playing, 2 paused.
    pub fn state(&self) -> u32 {
        if self.paused {
            2
        } else if self.playing {
            1
        } else {
            0
        }
    }

    /// Pull up to `max_frames` stereo frames, applying the configured volume.
    pub fn pull_samples(&mut self, max_frames: usize) -> Vec<i16> {
        if !self.playing || self.paused {
            return Vec::new();
        }
        if self.samples.is_empty() {
            self.underflow_frames = self.underflow_frames.saturating_add(max_frames as u64);
            return Vec::new();
        }

        let take = max_frames * 2;
        let volume_scale = self.volume as i32;
        let mut output = Vec::with_capacity(take.min(self.samples.len()));
        for _ in 0..take {
            let Some(sample) = self.samples.pop_front() else {
                break;
            };
            output.push(((sample as i32 * volume_scale) / 100) as i16);
        }
        output
    }

    /// Deterministic evidence about accepted and consumed audio.
    pub fn diagnostics(&self) -> AudioDiagnostics {
        let mut crc = crc32fast::Hasher::new();
        let mut sum_squares = 0.0f64;
        for sample in &self.samples {
            crc.update(&sample.to_le_bytes());
            let value = *sample as f64;
            sum_squares += value * value;
        }
        let rms_amplitude = (sum_squares / self.samples.len().max(1) as f64).sqrt() as f32;
        AudioDiagnostics {
            sample_rate: AUDIO_SAMPLE_RATE,
            channels: 2,
            volume: self.volume,
            submitted_bytes: self.submitted_bytes,
            decoded_frames: self.decoded_frames,
            nonzero_samples: self.nonzero_samples,
            rejected_writes: self.rejected_writes,
            underflow_frames: self.underflow_frames,
            max_buffered_frames: self.max_buffered_frames,
            pcm_crc32: crc.finalize(),
            rms_amplitude,
        }
    }
}

fn is_wav(data: &[u8]) -> bool {
    data.len() >= 12 && &data[..4] == b"RIFF" && &data[8..12] == b"WAVE"
}

fn is_mp3(data: &[u8]) -> bool {
    data.len() >= 2 && (data.starts_with(b"ID3") || (data[0] == 0xFF && data[1] & 0xE0 == 0xE0))
}

/// CBE audio resources carry a five-byte header: a u16 little-endian type
/// followed by a three-byte big-endian payload length.
fn audio_resource_payload(data: &[u8]) -> Option<(u16, &[u8])> {
    if data.len() < 5 {
        return None;
    }
    let payload_type = u16::from_le_bytes([data[0], data[1]]);
    let payload_len = ((data[2] as usize) << 16) | ((data[3] as usize) << 8) | data[4] as usize;
    if 5usize.saturating_add(payload_len) <= data.len() {
        Some((payload_type, &data[5..5 + payload_len]))
    } else {
        None
    }
}

/// Decode a Standard MIDI File into 44.1 kHz stereo PCM.
///
/// Notes are synthesized as a decaying sine tone with a short attack, which
/// approximates the feature-phone melody chip timbre well enough to hear the
/// original tunes.
fn decode_midi(data: &[u8]) -> Result<Vec<i16>> {
    if data.len() < 14 || &data[0..4] != b"MThd" {
        bail!("not a MIDI file");
    }
    let format = u16::from_be_bytes(data[8..10].try_into().unwrap());
    let track_count = u16::from_be_bytes(data[10..12].try_into().unwrap()) as usize;
    let division = u16::from_be_bytes(data[12..14].try_into().unwrap());
    if division == 0 || division & 0x8000 != 0 {
        bail!("unsupported MIDI division {division}");
    }
    let _ = format;
    let ticks_per_quarter = division as u64;

    #[derive(Debug, Clone, Copy)]
    struct NoteEvent {
        tick: u64,
        note: u8,
        velocity: u8,
        on: bool,
    }

    let mut events = Vec::new();
    let mut tempo_events = Vec::new();
    let mut cursor = 14usize;
    for _ in 0..track_count {
        if cursor + 8 > data.len() {
            bail!("truncated MIDI track header");
        }
        if &data[cursor..cursor + 4] != b"MTrk" {
            bail!("expected MTrk chunk");
        }
        let length = u32::from_be_bytes(data[cursor + 4..cursor + 8].try_into().unwrap()) as usize;
        let end = cursor.saturating_add(8).saturating_add(length);
        if end > data.len() {
            bail!("truncated MIDI track");
        }
        let mut pos = cursor + 8;
        let mut tick = 0u64;
        let mut running_status = 0u8;
        while pos < end {
            let (delta, used) = read_varint(&data[pos..end])?;
            pos += used;
            tick = tick.saturating_add(delta);
            if pos >= end {
                break;
            }
            let mut status = data[pos];
            if status & 0x80 == 0 {
                status = running_status;
            } else {
                pos += 1;
                running_status = status;
            }
            match status & 0xF0 {
                0xC0 | 0xD0 => {
                    pos = pos.saturating_add(1);
                }
                0x80 | 0x90 | 0xA0 | 0xB0 | 0xE0 => {
                    if pos + 2 > end {
                        break;
                    }
                    let data1 = data[pos];
                    let data2 = data[pos + 1];
                    pos += 2;
                    match status & 0xF0 {
                        0x80 => events.push(NoteEvent {
                            tick,
                            note: data1,
                            velocity: 0,
                            on: false,
                        }),
                        0x90 if data2 == 0 => events.push(NoteEvent {
                            tick,
                            note: data1,
                            velocity: 0,
                            on: false,
                        }),
                        0x90 => events.push(NoteEvent {
                            tick,
                            note: data1,
                            velocity: data2,
                            on: true,
                        }),
                        _ => {}
                    }
                }
                0xF0 if status == 0xFF => {
                    if pos >= end {
                        break;
                    }
                    let meta = data[pos];
                    pos += 1;
                    let (length, used) = read_varint(&data[pos..end])?;
                    let length = length as usize;
                    pos += used;
                    let body = pos;
                    pos = pos.saturating_add(length);
                    if body.saturating_add(length) > end {
                        break;
                    }
                    if meta == 0x51 && length >= 3 {
                        let tempo = ((data[body] as u32) << 16)
                            | ((data[body + 1] as u32) << 8)
                            | data[body + 2] as u32;
                        tempo_events.push((tick, tempo as u64));
                    }
                }
                0xF0 => {
                    let (length, used) = read_varint(&data[pos..end])?;
                    pos = pos.saturating_add(used).saturating_add(length as usize);
                }
                _ => break,
            }
        }
        cursor = end;
    }

    events.sort_by_key(|event| event.tick);
    if events.is_empty() {
        bail!("MIDI file has no note events");
    }
    if tempo_events.is_empty() {
        tempo_events.push((0, 500_000));
    }
    tempo_events.sort_by_key(|event| event.0);
    let max_tick = events.iter().map(|event| event.tick).max().unwrap_or(0);
    let end_tick = max_tick.saturating_add(ticks_per_quarter);

    let seconds_per_tick = |tempo: u64| tempo as f64 / ticks_per_quarter as f64 / 1_000_000.0;
    let tick_time = |tick: u64| -> f64 {
        let mut time = 0.0;
        let mut previous_tick = 0u64;
        for &(tempo_tick, tempo) in &tempo_events {
            if tempo_tick >= tick {
                break;
            }
            time += (tempo_tick - previous_tick) as f64 * seconds_per_tick(tempo);
            previous_tick = tempo_tick;
        }
        time += (tick - previous_tick) as f64
            * seconds_per_tick(tempo_events.last().map_or(500_000, |event| event.1));
        time
    };

    let total_seconds = tick_time(end_tick).clamp(0.05, 120.0);
    let total_samples = (total_seconds * AUDIO_SAMPLE_RATE as f64) as usize;
    let mut samples = vec![0.0f32; total_samples];
    let mut active: Vec<(f64, f64, f64, f32)> = Vec::new(); // (frequency, start, end, amplitude)
    let mut event_index = 0usize;

    for (sample_index, output) in samples.iter_mut().enumerate() {
        let time = sample_index as f64 / AUDIO_SAMPLE_RATE as f64;
        while event_index < events.len() {
            let event = events[event_index];
            let event_time = tick_time(event.tick);
            if event_time > time {
                break;
            }
            event_index += 1;
            let frequency = 440.0 * 2.0f64.powf((event.note as f64 - 69.0) / 12.0);
            if event.on {
                let amplitude = event.velocity as f32 / 127.0;
                active.push((frequency, time, time + 1.0, amplitude));
            } else {
                for voice in active.iter_mut() {
                    if (voice.0 - frequency).abs() < 0.5 && voice.2 == time + 1.0 {
                        voice.2 = time;
                    }
                }
            }
        }
        active.retain(|voice| voice.2 > time);
        let mut value = 0.0f32;
        for &(frequency, start, end, amplitude) in &active {
            let age = (time - start) as f32;
            let attack = (age / 0.005).min(1.0);
            let decay = (-age * 3.0).exp();
            let release = (((end - time) as f32) * 20.0).clamp(0.0, 1.0);
            let phase = 2.0 * std::f64::consts::PI * frequency * time;
            let fundamental = phase.sin() as f32;
            let harmonic = (2.0 * phase).sin() as f32 * 0.35;
            value += (fundamental + harmonic) * attack * decay * release * amplitude * 0.18f32;
        }
        *output = value.clamp(-1.0, 1.0);
    }

    let mut output = Vec::with_capacity(samples.len() * 2);
    for sample in samples {
        let value = (sample * 32767.0) as i16;
        output.push(value);
        output.push(value);
    }
    Ok(output)
}

fn read_varint(data: &[u8]) -> Result<(u64, usize)> {
    let mut value = 0u64;
    for (index, byte) in data.iter().take(4).enumerate() {
        value = (value << 7) | (byte & 0x7F) as u64;
        if byte & 0x80 == 0 {
            return Ok((value, index + 1));
        }
    }
    bail!("invalid MIDI variable-length quantity")
}

fn decode_wav(data: &[u8]) -> Result<(Vec<f32>, u32, u32)> {
    if !is_wav(data) {
        bail!("not a WAV file");
    }
    let mut cursor = 12usize;
    let mut channels = 0u32;
    let mut sample_rate = 0u32;
    let mut samples = Vec::new();
    let mut found_fmt = false;
    let mut found_data = false;

    while cursor + 8 <= data.len() {
        let size = u32::from_le_bytes(data[cursor + 4..cursor + 8].try_into().unwrap()) as usize;
        let body = cursor + 8;
        match &data[cursor..cursor + 4] {
            b"fmt " => {
                if body + 16 > data.len() {
                    bail!("truncated WAV fmt chunk");
                }
                let format = u16::from_le_bytes(data[body..body + 2].try_into().unwrap());
                if format != 1 {
                    bail!("unsupported WAV format {format}");
                }
                channels = u32::from(u16::from_le_bytes(
                    data[body + 2..body + 4].try_into().unwrap(),
                ));
                sample_rate = u32::from_le_bytes(data[body + 4..body + 8].try_into().unwrap());
                let bits = u16::from_le_bytes(data[body + 14..body + 16].try_into().unwrap());
                if bits != 16 {
                    bail!("unsupported WAV bit depth {bits}");
                }
                if channels == 0 || sample_rate == 0 {
                    bail!("WAV fmt chunk declares zero channels or sample rate");
                }
                found_fmt = true;
            }
            b"data" => {
                let available = data.len().saturating_sub(body);
                let sample_count = (size / 2).min(available / 2);
                for chunk in data[body..body + sample_count * 2].chunks_exact(2) {
                    samples.push(i16::from_le_bytes([chunk[0], chunk[1]]) as f32 / 32768.0);
                }
                found_data = true;
                break;
            }
            _ => {}
        }
        cursor = body.saturating_add(size).saturating_add(size & 1);
    }

    if !found_fmt || !found_data {
        bail!("WAV file is missing fmt or data chunk");
    }
    Ok((samples, channels, sample_rate))
}

fn decode_mp3(data: &[u8]) -> Result<Vec<i16>> {
    use symphonia::core::codecs::{audio::AudioDecoderOptions, CodecParameters};
    use symphonia::core::errors::Error as SymphoniaError;
    use symphonia::core::formats::{probe::Hint, FormatOptions, TrackType};
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;

    let source = Box::new(std::io::Cursor::new(data.to_vec()));
    let stream = MediaSourceStream::new(source, Default::default());
    let mut hint = Hint::new();
    hint.with_extension("mp3");
    let mut format = symphonia::default::get_probe()
        .probe(
            &hint,
            stream,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .context("failed to probe MP3 stream")?;
    let track = format
        .default_track(TrackType::Audio)
        .ok_or_else(|| anyhow::anyhow!("MP3 stream has no audio track"))?;
    let track_id = track.id;
    let codec_params = track
        .codec_params
        .clone()
        .ok_or_else(|| anyhow::anyhow!("MP3 track has no codec parameters"))?;
    let audio_codec_params = match codec_params {
        CodecParameters::Audio(params) => params,
        _ => bail!("MP3 track is not an audio track"),
    };
    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(&audio_codec_params, &AudioDecoderOptions::default())
        .context("failed to create MP3 decoder")?;

    let mut decoded = Vec::new();
    let mut sample_rate = 0u32;
    let mut channel_count = 0usize;
    let mut temp_buf = Vec::new();
    loop {
        let packet = match format.next_packet() {
            Ok(Some(packet)) => packet,
            Ok(None) => break,
            Err(SymphoniaError::IoError(error))
                if error.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(SymphoniaError::ResetRequired) => {
                bail!("MP3 stream changed tracks while decoding");
            }
            Err(error) => bail!("failed to read MP3 packet: {error}"),
        };
        if packet.track_id != track_id {
            continue;
        }
        let audio = match decoder.decode(&packet) {
            Ok(audio) => audio,
            Err(SymphoniaError::DecodeError(error)) => {
                log::debug!("Skipping malformed MP3 packet: {error}");
                continue;
            }
            Err(SymphoniaError::IoError(error))
                if error.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(error) => bail!("failed to decode MP3 packet: {error}"),
        };
        let spec = audio.spec();
        if sample_rate == 0 {
            sample_rate = spec.rate();
            channel_count = spec.channels().count();
        } else if sample_rate != spec.rate() || channel_count != spec.channels().count() {
            bail!("MP3 stream changed audio format while decoding");
        }
        temp_buf.clear();
        audio.copy_to_vec_interleaved(&mut temp_buf);
        decoded.extend_from_slice(&temp_buf);
    }

    if sample_rate == 0 || channel_count == 0 || decoded.is_empty() {
        bail!("MP3 stream decoded to no samples");
    }
    Ok(resample_to_stereo(
        &decoded,
        channel_count,
        sample_rate,
        AUDIO_SAMPLE_RATE,
    ))
}

fn resample_to_stereo(
    samples: &[f32],
    channels: usize,
    input_rate: u32,
    output_rate: u32,
) -> Vec<i16> {
    if samples.is_empty() || channels == 0 || input_rate == 0 || output_rate == 0 {
        return Vec::new();
    }
    let input_frames = samples.len() / channels;
    if input_frames == 0 {
        return Vec::new();
    }
    let output_frames =
        (input_frames as u64 * output_rate as u64).div_ceil(input_rate as u64) as usize;
    let mut output = Vec::with_capacity(output_frames * 2);

    for output_frame in 0..output_frames {
        let position = output_frame as u64 * input_rate as u64;
        let source_frame = (position / output_rate as u64) as usize;
        let next_frame = (source_frame + 1).min(input_frames - 1);
        let fraction = (position % output_rate as u64) as f32 / output_rate as f32;

        for channel in 0..2 {
            let source_channel = channel.min(channels - 1);
            let first = samples[source_frame.min(input_frames - 1) * channels + source_channel];
            let second = samples[next_frame * channels + source_channel];
            let sample = first + (second - first) * fraction;
            output.push((sample * 32767.0).clamp(i16::MIN as f32, i16::MAX as f32) as i16);
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine_wav(frames: u32, rate: u32) -> Vec<u8> {
        let data_len = frames as usize * 2;
        let mut out = Vec::new();
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&(36 + data_len as u32).to_le_bytes());
        out.extend_from_slice(b"WAVE");
        out.extend_from_slice(b"fmt ");
        out.extend_from_slice(&16u32.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&rate.to_le_bytes());
        out.extend_from_slice(&(rate * 2).to_le_bytes());
        out.extend_from_slice(&2u16.to_le_bytes());
        out.extend_from_slice(&16u16.to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&(data_len as u32).to_le_bytes());
        for index in 0..frames {
            let value = ((index as f32 / frames as f32 * std::f32::consts::TAU * 4.0).sin()
                * 8000.0) as i16;
            out.extend_from_slice(&value.to_le_bytes());
        }
        out
    }

    #[test]
    fn wav_decode_resamples_to_stereo_output_rate() {
        let wav = sine_wav(100, 8000);
        let (samples, channels, rate) = decode_wav(&wav).unwrap();
        assert_eq!(channels, 1);
        assert_eq!(rate, 8000);
        assert_eq!(samples.len(), 100);

        let output = resample_to_stereo(&samples, channels as usize, rate, AUDIO_SAMPLE_RATE);
        let expected_frames = (100u64 * AUDIO_SAMPLE_RATE as u64).div_ceil(8000) as usize;
        assert_eq!(output.len(), expected_frames * 2);
        assert!(output.iter().any(|sample| *sample != 0));
    }

    #[test]
    fn volume_scales_pulled_output() {
        let wav = sine_wav(200, 8000);
        let mut loud = AudioEngine::new();
        loud.play_bytes(&wav).unwrap();
        let loud_samples = loud.pull_samples(100);

        let mut quiet = AudioEngine::new();
        quiet.set_volume(50);
        quiet.play_bytes(&wav).unwrap();
        let quiet_samples = quiet.pull_samples(100);

        assert_eq!(loud_samples.len(), quiet_samples.len());
        for (left, right) in loud_samples.iter().zip(quiet_samples.iter()) {
            assert_eq!(*right, (*left as i32 * 50 / 100) as i16);
        }
    }

    #[test]
    fn pause_resume_stop_control_state() {
        let wav = sine_wav(200, 8000);
        let mut engine = AudioEngine::new();
        assert_eq!(engine.state(), 0);
        engine.play_bytes(&wav).unwrap();
        assert_eq!(engine.state(), 1);
        engine.pause();
        assert_eq!(engine.state(), 2);
        assert!(engine.pull_samples(10).is_empty());
        engine.resume();
        assert_eq!(engine.state(), 1);
        assert!(!engine.pull_samples(10).is_empty());
        engine.stop();
        assert_eq!(engine.state(), 0);
    }

    #[test]
    fn diagnostics_are_deterministic_and_reject_unknown_data() {
        let wav = sine_wav(300, 8000);
        let mut first = AudioEngine::new();
        first.play_bytes(&wav).unwrap();
        first.pull_samples(100);
        let mut second = AudioEngine::new();
        second.play_bytes(&wav).unwrap();
        second.pull_samples(100);
        assert_eq!(first.diagnostics(), second.diagnostics());
        assert!(first.diagnostics().nonzero_samples > 0);

        let mut rejected = AudioEngine::new();
        assert!(rejected.play_bytes(b"not audio").is_err());
        assert_eq!(rejected.diagnostics().rejected_writes, 1);
    }

    #[test]
    #[ignore = "requires a local MP3 file (set NICAI_MP3_PATH)"]
    fn decodes_real_mp3_from_path() {
        let path = std::env::var_os("NICAI_MP3_PATH").expect("NICAI_MP3_PATH is not set");
        let data = std::fs::read(&path).expect("failed to read MP3 file");
        let mut engine = AudioEngine::new();
        engine.play_bytes(&data).expect("failed to decode MP3");
        assert!(engine.pull_samples(44_100).len() >= 2);
        let diagnostics = engine.diagnostics();
        assert!(diagnostics.decoded_frames > 0);
        assert!(diagnostics.nonzero_samples > 0);
    }

    fn tiny_midi() -> Vec<u8> {
        // Format 0, one track, 48 ticks per quarter note.
        let mut midi = Vec::new();
        midi.extend_from_slice(b"MThd");
        midi.extend_from_slice(&6u32.to_be_bytes());
        midi.extend_from_slice(&0u16.to_be_bytes());
        midi.extend_from_slice(&1u16.to_be_bytes());
        midi.extend_from_slice(&48u16.to_be_bytes());

        let mut track = Vec::new();
        track.extend_from_slice(&[0x00, 0xFF, 0x51, 0x03, 0x07, 0xA1, 0x20]); // tempo
        track.extend_from_slice(&[0x00, 0x90, 0x3C, 0x64]); // note on C4
        track.extend_from_slice(&[0x30, 0x80, 0x3C, 0x00]); // note off after 48 ticks
        track.extend_from_slice(&[0x00, 0xFF, 0x2F, 0x00]); // end of track

        midi.extend_from_slice(b"MTrk");
        midi.extend_from_slice(&(track.len() as u32).to_be_bytes());
        midi.extend_from_slice(&track);
        midi
    }

    #[test]
    fn audio_resource_payload_parses_header() {
        let mut data = vec![0x0A, 0x00, 0x00, 0x00, 0x03];
        data.extend_from_slice(b"abc");
        assert_eq!(
            audio_resource_payload(&data),
            Some((0x000A, b"abc".as_slice()))
        );
        assert_eq!(audio_resource_payload(b"1234"), None);
    }

    #[test]
    fn decodes_small_midi_to_audible_samples() {
        let decoded = decode_midi(&tiny_midi()).unwrap();
        assert!(decoded.len() >= 2);
        assert!(decoded.iter().any(|sample| *sample != 0));
    }

    #[test]
    fn play_bytes_handles_midi_resource_header_and_repeats() {
        let midi = tiny_midi();
        let mut resource = vec![0x0A, 0x00, 0x00, 0x00, midi.len() as u8];
        resource.extend_from_slice(&midi);

        let mut once = AudioEngine::new();
        once.play_bytes(&resource).unwrap();
        let mut twice = AudioEngine::new();
        twice.play_bytes_repeats(&resource, 1).unwrap();

        assert!(once.diagnostics().decoded_frames > 0);
        assert_eq!(
            twice.diagnostics().decoded_frames,
            once.diagnostics().decoded_frames * 2
        );
        assert_eq!(once.diagnostics().rejected_writes, 0);
    }
}
