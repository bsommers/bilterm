//! Zero-dependency RIFF/WAVE S16_LE 16 kHz mono encoder/decoder and RMS analysis.
//!
//! Provides in-memory framing of 16 kHz 16-bit signed mono PCM audio streams,
//! dBFS level calculation for VAD and live VU meters, and secure ephemeral file
//! scrubbing for CLI-based STT backends (`SEC-R01`, `STT-02`).

use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

pub const STT_SAMPLE_RATE: u32 = 16_000;
pub const STT_CHANNELS: u16 = 1;
pub const STT_BITS_PER_SAMPLE: u16 = 16;
pub const STT_BYTES_PER_SAMPLE: usize = 2;
pub const WAV_HEADER_LEN: usize = 44;

/// Encodes raw 16 kHz 16-bit mono little-endian PCM bytes into a 44-byte framed RIFF/WAVE buffer.
pub fn encode_pcm_bytes_to_wav(pcm_s16le: &[u8]) -> Vec<u8> {
    let data_len = pcm_s16le.len() as u32;
    let riff_chunk_size = 36u32.saturating_add(data_len);
    let byte_rate = STT_SAMPLE_RATE * u32::from(STT_CHANNELS) * u32::from(STT_BITS_PER_SAMPLE) / 8;
    let block_align = STT_CHANNELS * (STT_BITS_PER_SAMPLE / 8);

    let mut out = Vec::with_capacity(WAV_HEADER_LEN + pcm_s16le.len());
    // 0..4: ChunkID "RIFF"
    out.extend_from_slice(b"RIFF");
    // 4..8: ChunkSize
    out.extend_from_slice(&riff_chunk_size.to_le_bytes());
    // 8..12: Format "WAVE"
    out.extend_from_slice(b"WAVE");
    // 12..16: Subchunk1ID "fmt "
    out.extend_from_slice(b"fmt ");
    // 16..20: Subchunk1Size (16 for PCM)
    out.extend_from_slice(&16u32.to_le_bytes());
    // 20..22: AudioFormat (1 = PCM)
    out.extend_from_slice(&1u16.to_le_bytes());
    // 22..24: NumChannels (1 = mono)
    out.extend_from_slice(&STT_CHANNELS.to_le_bytes());
    // 24..28: SampleRate (16000)
    out.extend_from_slice(&STT_SAMPLE_RATE.to_le_bytes());
    // 28..32: ByteRate (32000)
    out.extend_from_slice(&byte_rate.to_le_bytes());
    // 32..34: BlockAlign (2)
    out.extend_from_slice(&block_align.to_le_bytes());
    // 34..36: BitsPerSample (16)
    out.extend_from_slice(&STT_BITS_PER_SAMPLE.to_le_bytes());
    // 36..40: Subchunk2ID "data"
    out.extend_from_slice(b"data");
    // 40..44: Subchunk2Size
    out.extend_from_slice(&data_len.to_le_bytes());
    // 44..: Data
    out.extend_from_slice(pcm_s16le);
    out
}

/// Encodes 16-bit signed mono PCM samples into a 44-byte framed RIFF/WAVE buffer.
pub fn encode_wav_s16le_mono(samples: &[i16]) -> Vec<u8> {
    let mut pcm = Vec::with_capacity(samples.len() * STT_BYTES_PER_SAMPLE);
    for &s in samples {
        pcm.extend_from_slice(&s.to_le_bytes());
    }
    encode_pcm_bytes_to_wav(&pcm)
}

/// Parses a 44-byte (or extended) RIFF/WAVE mono S16_LE buffer into `i16` samples.
pub fn parse_wav_s16le_mono(wav_bytes: &[u8]) -> Result<Vec<i16>, String> {
    if wav_bytes.len() < WAV_HEADER_LEN {
        return Err("WAV buffer too short for header".to_string());
    }
    if &wav_bytes[0..4] != b"RIFF" || &wav_bytes[8..12] != b"WAVE" {
        return Err("Not a valid RIFF/WAVE buffer".to_string());
    }

    // Scan for "fmt " and "data" chunks
    let mut offset = 12;
    let mut sample_rate = 0u32;
    let mut channels = 0u16;
    let mut bits_per_sample = 0u16;
    let mut data_slice: Option<&[u8]> = None;

    while offset + 8 <= wav_bytes.len() {
        let chunk_id = &wav_bytes[offset..offset + 4];
        let chunk_size = u32::from_le_bytes([
            wav_bytes[offset + 4],
            wav_bytes[offset + 5],
            wav_bytes[offset + 6],
            wav_bytes[offset + 7],
        ]) as usize;
        let chunk_data_start = offset + 8;
        let chunk_data_end = (chunk_data_start + chunk_size).min(wav_bytes.len());

        if chunk_id == b"fmt " && chunk_size >= 16 && chunk_data_end <= wav_bytes.len() {
            channels = u16::from_le_bytes([wav_bytes[chunk_data_start + 2], wav_bytes[chunk_data_start + 3]]);
            sample_rate = u32::from_le_bytes([
                wav_bytes[chunk_data_start + 4],
                wav_bytes[chunk_data_start + 5],
                wav_bytes[chunk_data_start + 6],
                wav_bytes[chunk_data_start + 7],
            ]);
            bits_per_sample = u16::from_le_bytes([wav_bytes[chunk_data_start + 14], wav_bytes[chunk_data_start + 15]]);
        } else if chunk_id == b"data" {
            data_slice = Some(&wav_bytes[chunk_data_start..chunk_data_end]);
            break;
        }

        offset = chunk_data_start + chunk_size;
        // Chunks are 2-byte aligned
        if !chunk_size.is_multiple_of(2) {
            offset += 1;
        }
    }

    let Some(data) = data_slice else {
        return Err("No 'data' chunk found in WAV payload".to_string());
    };
    if sample_rate != 0 && sample_rate != STT_SAMPLE_RATE {
        tracing::warn!("WAV sample rate is {} Hz, expected 16000 Hz", sample_rate);
    }
    if channels > 1 {
        return Err(format!("Expected mono audio, found {channels} channels"));
    }
    if bits_per_sample != 0 && bits_per_sample != 16 {
        return Err(format!("Expected 16-bit audio, found {bits_per_sample}-bit"));
    }

    let mut samples = Vec::with_capacity(data.len() / 2);
    for chunk in data.chunks_exact(2) {
        samples.push(i16::from_le_bytes([chunk[0], chunk[1]]));
    }
    Ok(samples)
}

/// Computes the Root-Mean-Square (RMS) level of audio samples in dBFS (-96.0..=0.0).
pub fn compute_rms_dbfs(samples: &[i16]) -> f32 {
    if samples.is_empty() {
        return -96.0;
    }
    let mut sum_sq: f64 = 0.0;
    for &sample in samples {
        let s = sample as f64;
        sum_sq += s * s;
    }
    let mean_sq = sum_sq / (samples.len() as f64);
    let rms = mean_sq.sqrt();
    if rms <= 1.0e-5 {
        return -96.0;
    }
    let dbfs = 20.0 * (rms / 32768.0).log10();
    (dbfs as f32).clamp(-96.0, 0.0)
}

/// RAII guard that manages a secure 0600 temporary WAV file for CLI backends.
///
/// On drop, the file is overwritten with zero bytes before unlinking (`SEC-R01`).
pub struct ScrubbedTempWav {
    path: PathBuf,
    len: usize,
}

impl ScrubbedTempWav {
    /// Writes `wav_bytes` into a newly created 0600 file inside the private runtime STT directory.
    pub fn create(wav_bytes: &[u8]) -> Result<Self, String> {
        let runtime_dir = crate::secure_fs::private_runtime_dir()
            .map_err(|e| format!("Failed to resolve private runtime dir: {e}"))?;
        let stt_dir = crate::secure_fs::private_dir_in(&runtime_dir, "stt")
            .map_err(|e| format!("Failed to create private stt dir: {e}"))?;
        let file_name = format!("bilterm-stt-{}.wav", crate::secure_fs::unique_suffix());
        let file_path = stt_dir.join(file_name);

        let mut file = crate::secure_fs::create_private_file(&file_path)
            .map_err(|e| format!("Failed to create private WAV file: {e}"))?;
        file.write_all(wav_bytes)
            .and_then(|()| file.sync_all())
            .map_err(|e| format!("Failed to write private WAV file: {e}"))?;

        Ok(Self {
            path: file_path,
            len: wav_bytes.len(),
        })
    }

    /// Path to the temporary WAV file on disk.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for ScrubbedTempWav {
    fn drop(&mut self) {
        if self.path.exists() {
            // Overwrite file content with zeros before removal
            if let Ok(mut f) = File::options().write(true).open(&self.path) {
                let zeros = vec![0u8; self.len.min(65536)];
                let mut written = 0;
                while written < self.len {
                    let to_write = (self.len - written).min(zeros.len());
                    if f.write_all(&zeros[..to_write]).is_err() {
                        break;
                    }
                    written += to_write;
                }
                let _ = f.sync_all();
            }
            let _ = std::fs::remove_file(&self.path);
        }
    }
}
