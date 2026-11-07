use std::num::NonZeroU32;
use std::path::{Path, PathBuf};

use mp4::Mp4Reader;
use tracing::{info, warn};

// Audio re-encoding for HE-AAC compatibility
use mp3lame_encoder::{Builder as LameBuilder, DualPcm, FlushNoGap};
use symphonia::core::audio::Channels;
use symphonia::core::codecs::CodecParameters;
use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;

use crate::error::{DownloadError, Error, Result};

// ============================================================================
// Audio Extraction from MP4 to AAC (ADTS format)
// ============================================================================

/// Extract audio track from an MP4 file and save it as AAC with ADTS headers.
///
/// This function reads an MP4 file containing video+audio, extracts the AAC
/// audio samples, and writes them with ADTS headers. ADTS (Audio Data Transport
/// Stream) is a streaming format for AAC that's widely supported by players
/// including VLC and most MP3 players that support AAC/M4A.
///
/// The output file uses .aac extension which is recognized by most players.
/// HE-AAC sources are re-encoded to MP3 instead, see [`reencode_aac_to_mp3`].
///
/// Returns the path of the file actually written, which has the `.aac` or `.mp3`
/// extension regardless of the extension of `output_path`.
///
/// # Errors
///
/// Returns an error if the file cannot be read, has no audio track, or writing fails.
#[allow(
    clippy::too_many_lines,
    reason = "audio extraction requires sequential steps"
)]
pub(super) fn extract_audio_to_m4a(
    input_path: &Path,
    output_path: &Path,
    title: &str,
) -> Result<PathBuf> {
    use std::io::{BufReader, BufWriter, Seek, SeekFrom, Write};

    info!(
        "Extracting audio from {:?} to {:?}",
        input_path, output_path
    );

    // Open the input MP4 file
    let input_file = std::fs::File::open(input_path).map_err(|e| {
        Error::Download(DownloadError::AudioExtractionFailed {
            title: title.to_string(),
            reason: format!("Failed to open input file: {e}"),
        })
    })?;

    let file_size = input_file.metadata().map_or(0, |m| m.len());
    let mut reader = BufReader::new(input_file);

    // Read the MP4 header
    let mp4_reader = Mp4Reader::read_header(&mut reader, file_size).map_err(|e| {
        Error::Download(DownloadError::AudioExtractionFailed {
            title: title.to_string(),
            reason: format!("Failed to read MP4 header: {e}"),
        })
    })?;

    // Find the audio track
    let audio_track = mp4_reader
        .tracks()
        .values()
        .find(|t| t.track_type().is_ok_and(|tt| tt == mp4::TrackType::Audio))
        .ok_or_else(|| {
            Error::Download(DownloadError::AudioExtractionFailed {
                title: title.to_string(),
                reason: "No audio track found in MP4 file".to_string(),
            })
        })?;

    let audio_track_id = audio_track.track_id();
    let sample_count = audio_track.sample_count();
    let box_type = audio_track.box_type();

    info!(
        "Found audio track {} with {} samples, box_type: {:?}, sample rate: {:?}Hz, channels: {:?}",
        audio_track_id,
        sample_count,
        box_type,
        audio_track.sample_freq_index(),
        audio_track.channel_config()
    );

    // Get AAC configuration for ADTS header generation
    let profile = audio_track.audio_profile().map_err(|e| {
        Error::Download(DownloadError::AudioExtractionFailed {
            title: title.to_string(),
            reason: format!(
                "Audio track is not AAC (box_type: {box_type:?}). Only AAC audio can be extracted. Error: {e}"
            ),
        })
    })?;

    let freq_index = audio_track.sample_freq_index().map_err(|e| {
        Error::Download(DownloadError::AudioExtractionFailed {
            title: title.to_string(),
            reason: format!("Failed to get sample frequency from track: {e}"),
        })
    })?;

    let chan_conf = audio_track.channel_config().map_err(|e| {
        Error::Download(DownloadError::AudioExtractionFailed {
            title: title.to_string(),
            reason: format!("Failed to get channel config from track: {e}"),
        })
    })?;

    // Log the actual parameters for debugging
    info!(
        "AAC track parameters - profile: {:?}, freq: {:?}, channels: {:?}",
        profile, freq_index, chan_conf
    );

    // Check if this is HE-AAC (SBR) or HE-AACv2 (PS) which many MP3 players don't support
    // These need to be re-encoded to MP3 for compatibility
    let is_he_aac = matches!(
        profile,
        mp4::AudioObjectType::SpectralBandReplication | mp4::AudioObjectType::ParametricStereo
    );

    if is_he_aac {
        info!(
            "Detected HE-AAC profile {:?} - will re-encode to MP3 for compatibility",
            profile
        );
        // Drop the mp4_reader to release the file handle
        drop(mp4_reader);
        // Re-encode HE-AAC to MP3 using symphonia + LAME
        return reencode_aac_to_mp3(input_path, output_path, title);
    }

    // Convert to numeric values for ADTS header
    // ADTS profile is AudioObjectType - 1 (AAC-LC = 2 in AOT, but 1 in ADTS)
    let adts_profile: u8 = match profile {
        mp4::AudioObjectType::AacMain => 0,
        mp4::AudioObjectType::AacLowComplexity => 1,
        mp4::AudioObjectType::AacScalableSampleRate => 2,
        mp4::AudioObjectType::AacLongTermPrediction => 3,
        mp4::AudioObjectType::SpectralBandReplication
        | mp4::AudioObjectType::AACScalable
        | mp4::AudioObjectType::TwinVQ
        | mp4::AudioObjectType::CodeExcitedLinearPrediction
        | mp4::AudioObjectType::HarmonicVectorExcitationCoding
        | mp4::AudioObjectType::TextToSpeechtInterface
        | mp4::AudioObjectType::MainSynthetic
        | mp4::AudioObjectType::WavetableSynthesis
        | mp4::AudioObjectType::GeneralMIDI
        | mp4::AudioObjectType::AlgorithmicSynthesis
        | mp4::AudioObjectType::ErrorResilientAacLowComplexity
        | mp4::AudioObjectType::ErrorResilientAacLongTermPrediction
        | mp4::AudioObjectType::ErrorResilientAacScalable
        | mp4::AudioObjectType::ErrorResilientAacTwinVQ
        | mp4::AudioObjectType::ErrorResilientAacBitSlicedArithmeticCoding
        | mp4::AudioObjectType::ErrorResilientAacLowDelay
        | mp4::AudioObjectType::ErrorResilientCodeExcitedLinearPrediction
        | mp4::AudioObjectType::ErrorResilientHarmonicVectorExcitationCoding
        | mp4::AudioObjectType::ErrorResilientHarmonicIndividualLinesNoise
        | mp4::AudioObjectType::ErrorResilientParametric
        | mp4::AudioObjectType::SinuSoidalCoding
        | mp4::AudioObjectType::ParametricStereo
        | mp4::AudioObjectType::MpegSurround
        | mp4::AudioObjectType::MpegLayer1
        | mp4::AudioObjectType::MpegLayer2
        | mp4::AudioObjectType::MpegLayer3
        | mp4::AudioObjectType::DirectStreamTransfer
        | mp4::AudioObjectType::AudioLosslessCoding
        | mp4::AudioObjectType::ScalableLosslessCoding
        | mp4::AudioObjectType::ScalableLosslessCodingNoneCore
        | mp4::AudioObjectType::ErrorResilientAacEnhancedLowDelay
        | mp4::AudioObjectType::SymbolicMusicRepresentationSimple
        | mp4::AudioObjectType::SymbolicMusicRepresentationMain
        | mp4::AudioObjectType::UnifiedSpeechAudioCoding
        | mp4::AudioObjectType::SpatialAudioObjectCoding
        | mp4::AudioObjectType::LowDelayMpegSurround
        | mp4::AudioObjectType::SpatialAudioObjectCodingDialogueEnhancement
        | mp4::AudioObjectType::AudioSync => {
            warn!(
                "Unsupported AAC profile {:?}, defaulting to AAC-LC",
                profile
            );
            1 // Default to AAC-LC
        }
    };

    // Sample frequency index
    let adts_freq_index: u8 = match freq_index {
        mp4::SampleFreqIndex::Freq96000 => 0,
        mp4::SampleFreqIndex::Freq88200 => 1,
        mp4::SampleFreqIndex::Freq64000 => 2,
        mp4::SampleFreqIndex::Freq48000 => 3,
        mp4::SampleFreqIndex::Freq44100 => 4,
        mp4::SampleFreqIndex::Freq32000 => 5,
        mp4::SampleFreqIndex::Freq24000 => 6,
        mp4::SampleFreqIndex::Freq22050 => 7,
        mp4::SampleFreqIndex::Freq16000 => 8,
        mp4::SampleFreqIndex::Freq12000 => 9,
        mp4::SampleFreqIndex::Freq11025 => 10,
        mp4::SampleFreqIndex::Freq8000 => 11,
        mp4::SampleFreqIndex::Freq7350 => 12,
    };

    // Channel configuration
    let adts_chan_conf: u8 = match chan_conf {
        mp4::ChannelConfig::Mono => 1,
        mp4::ChannelConfig::Stereo => 2,
        mp4::ChannelConfig::Three => 3,
        mp4::ChannelConfig::Four => 4,
        mp4::ChannelConfig::Five => 5,
        mp4::ChannelConfig::FiveOne => 6,
        mp4::ChannelConfig::SevenOne => 7,
    };

    // Create output AAC file with ADTS headers
    // Change extension from .m4a to .aac for ADTS format
    let aac_output_path = output_path.with_extension("aac");
    let output_file = std::fs::File::create(&aac_output_path).map_err(|e| {
        Error::Download(DownloadError::AudioExtractionFailed {
            title: title.to_string(),
            reason: format!("Failed to create output file: {e}"),
        })
    })?;
    let mut writer = BufWriter::new(output_file);

    // Re-read samples from the MP4
    drop(mp4_reader);
    reader.seek(SeekFrom::Start(0)).map_err(|e| {
        Error::Download(DownloadError::AudioExtractionFailed {
            title: title.to_string(),
            reason: format!("Failed to seek to start: {e}"),
        })
    })?;

    let mut mp4_reader = Mp4Reader::read_header(&mut reader, file_size).map_err(|e| {
        Error::Download(DownloadError::AudioExtractionFailed {
            title: title.to_string(),
            reason: format!("Failed to re-read MP4 header: {e}"),
        })
    })?;

    // Write each sample with an ADTS header
    for sample_idx in 1..=sample_count {
        let sample = mp4_reader
            .read_sample(audio_track_id, sample_idx)
            .map_err(|e| {
                Error::Download(DownloadError::AudioExtractionFailed {
                    title: title.to_string(),
                    reason: format!("Failed to read sample {sample_idx}: {e}"),
                })
            })?;

        if let Some(sample) = sample {
            // Create ADTS header (7 bytes) for this frame
            let frame_len = sample.bytes.len() + 7; // Include header length
            let frame_len_u16: u16 = frame_len.try_into().map_err(|_err| {
                Error::Download(DownloadError::AudioExtractionFailed {
                    title: title.to_string(),
                    reason: format!("Frame length {frame_len} exceeds u16 max"),
                })
            })?;
            let adts_header =
                create_adts_header(adts_profile, adts_freq_index, adts_chan_conf, frame_len_u16);

            // Write ADTS header + raw AAC frame
            writer.write_all(&adts_header).map_err(|e| {
                Error::Download(DownloadError::AudioExtractionFailed {
                    title: title.to_string(),
                    reason: format!("Failed to write ADTS header for sample {sample_idx}: {e}"),
                })
            })?;

            writer.write_all(&sample.bytes).map_err(|e| {
                Error::Download(DownloadError::AudioExtractionFailed {
                    title: title.to_string(),
                    reason: format!("Failed to write sample {sample_idx}: {e}"),
                })
            })?;
        }
    }

    writer.flush().map_err(|e| {
        Error::Download(DownloadError::AudioExtractionFailed {
            title: title.to_string(),
            reason: format!("Failed to flush output file: {e}"),
        })
    })?;

    info!(
        "Successfully extracted {} audio samples to AAC (ADTS format): {:?}",
        sample_count, aac_output_path
    );
    Ok(aac_output_path)
}

/// Create a 7-byte ADTS header for an AAC frame.
///
/// ADTS header structure:
/// - Syncword: 12 bits (0xFFF)
/// - ID: 1 bit (0 = MPEG-4, 1 = MPEG-2)
/// - Layer: 2 bits (always 0)
/// - Protection absent: 1 bit (1 = no CRC)
/// - Profile: 2 bits (0 = Main, 1 = LC, 2 = SSR, 3 = LTP)
/// - Sampling frequency index: 4 bits
/// - Private bit: 1 bit
/// - Channel configuration: 3 bits
/// - Original/copy: 1 bit
/// - Home: 1 bit
/// - Copyright ID bit: 1 bit
/// - Copyright ID start: 1 bit
/// - Frame length: 13 bits (including header)
/// - Buffer fullness: 11 bits (0x7FF = VBR)
/// - Number of AAC frames - 1: 2 bits (0 = 1 frame)
#[allow(
    clippy::cast_possible_truncation,
    reason = "ADTS header fields are intentionally truncated to fit bit widths"
)]
const fn create_adts_header(profile: u8, freq_index: u8, chan_conf: u8, frame_len: u16) -> [u8; 7] {
    let mut header = [0u8; 7];

    // Byte 0: Syncword high byte (0xFF)
    header[0] = 0xFF;

    // Byte 1: Syncword low nibble (0xF) + ID (0) + Layer (00) + Protection absent (1)
    header[1] = 0xF1; // 1111 0001

    // Byte 2: Profile (2 bits) + Freq index (4 bits) + Private bit (1) + Channel high bit (1)
    header[2] = ((profile & 0x03) << 6) | ((freq_index & 0x0F) << 2) | ((chan_conf >> 2) & 0x01);

    // Byte 3: Channel low bits (2) + Original (1) + Home (1) + Copyright ID (1) + Copyright start (1) + Frame length high (2)
    header[3] = ((chan_conf & 0x03) << 6) | ((frame_len >> 11) as u8 & 0x03);

    // Byte 4: Frame length middle (8 bits)
    header[4] = (frame_len >> 3) as u8;

    // Byte 5: Frame length low (3 bits) + Buffer fullness high (5 bits)
    header[5] = ((frame_len as u8 & 0x07) << 5) | 0x1F; // 0x1F = buffer fullness high bits (VBR)

    // Byte 6: Buffer fullness low (6 bits) + Number of frames - 1 (2 bits)
    header[6] = 0xFC; // 1111 1100 = buffer fullness low (0x3F) + 0 frames

    header
}

/// MPEG-4 audio object type of AAC-LC (ISO/IEC 14496-3, table 1.17).
const AOT_AAC_LC: u8 = 2;
/// MPEG-4 audio object type of SBR, i.e. HE-AAC wrapping an AAC-LC core.
const AOT_SBR: u8 = 5;
/// MPEG-4 audio object type of PS, i.e. HE-AACv2 wrapping an AAC-LC core.
const AOT_PS: u8 = 29;

/// Sampling frequencies indexed by the 4-bit `samplingFrequencyIndex` of an
/// `AudioSpecificConfig` (ISO/IEC 14496-3, table 1.18). Indices 13 and 14 are
/// reserved and 15 escapes to an explicit 24-bit frequency.
const AAC_SAMPLE_RATES: [u32; 13] = [
    96_000, 88_200, 64_000, 48_000, 44_100, 32_000, 24_000, 22_050, 16_000, 12_000, 11_025, 8_000,
    7_350,
];

/// The AAC-LC core of an HE-AAC stream, described as a plain AAC-LC configuration.
struct AacLcCore {
    /// Two-byte AAC-LC `AudioSpecificConfig` for the core.
    config: Box<[u8]>,
    /// Sample rate of the core, half the nominal HE-AAC output rate.
    sample_rate: u32,
}

/// Rewrite an HE-AAC (SBR) or HE-AACv2 (SBR + PS) `AudioSpecificConfig` so that it
/// describes only the embedded AAC-LC core.
///
/// symphonia implements AAC-LC only and rejects HE-AAC outright. The SBR and PS
/// payloads live in fill elements that the LC decoder skips, so decoding the core
/// works and yields audio at the core sample rate without the reconstructed high
/// band. That is a quality loss, but a muffled MP3 beats a failed download.
///
/// Returns `None` when the config is not HE-AAC, or uses an escaped sample rate or
/// a PCE-defined channel layout, so the caller keeps the original parameters.
fn aac_lc_core_config(asc: &[u8]) -> Option<AacLcCore> {
    let (&byte0, &byte1) = (asc.first()?, asc.get(1)?);
    let object_type = byte0 >> 3;
    if object_type != AOT_SBR && object_type != AOT_PS {
        return None;
    }
    let freq_index = ((byte0 & 0x07) << 1) | (byte1 >> 7);
    let chan_conf = (byte1 >> 3) & 0x0F;
    let sample_rate = *AAC_SAMPLE_RATES.get(usize::from(freq_index))?;
    if chan_conf == 0 {
        return None;
    }
    // AOT (5 bits) | samplingFrequencyIndex (4) | channelConfiguration (4) |
    // GASpecificConfig: frameLengthFlag, dependsOnCoreCoder, extensionFlag (3 zero bits)
    let config = Box::new([
        (AOT_AAC_LC << 3) | (freq_index >> 1),
        ((freq_index & 0x01) << 7) | (chan_conf << 3),
    ]);
    Some(AacLcCore {
        config,
        sample_rate,
    })
}

/// Re-encode AAC (including HE-AAC) to MP3 for compatibility with basic MP3 players.
///
/// This function uses symphonia to decode AAC audio and LAME to encode to MP3.
/// It's used when the source audio is HE-AAC (SBR/PS) which many portable
/// MP3 players don't support. symphonia cannot decode SBR, so only the AAC-LC
/// core of such streams is decoded; see [`aac_lc_core_config`].
///
/// Returns the path of the MP3 written, `output_path` with an `.mp3` extension.
#[allow(
    clippy::too_many_lines,
    reason = "audio re-encoding requires sequential steps"
)]
fn reencode_aac_to_mp3(input_path: &Path, output_path: &Path, title: &str) -> Result<PathBuf> {
    use std::fs::File;
    use std::io::Write;

    info!(
        "Re-encoding AAC to MP3: {:?} -> {:?}",
        input_path, output_path
    );

    // Open and probe the input file
    let file = File::open(input_path).map_err(|e| {
        Error::Download(DownloadError::AudioExtractionFailed {
            title: title.to_string(),
            reason: format!("Failed to open input file: {e}"),
        })
    })?;

    let mss = MediaSourceStream::new(
        Box::new(file),
        symphonia::core::io::MediaSourceStreamOptions::default(),
    );

    let mut hint = Hint::new();
    if let Some(ext) = input_path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let mut format = symphonia::default::get_probe()
        .probe(
            &hint,
            mss,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .map_err(|e| {
            Error::Download(DownloadError::AudioExtractionFailed {
                title: title.to_string(),
                reason: format!("Failed to probe audio format: {e}"),
            })
        })?;

    // Find an audio track - MP4 files from YouTube have both video and audio tracks.
    // Prefer a track with a known audio codec, then fall back to any audio track.
    let track = format
        .first_track_known_codec(TrackType::Audio)
        .or_else(|| format.first_track(TrackType::Audio))
        .ok_or_else(|| {
            Error::Download(DownloadError::AudioExtractionFailed {
                title: title.to_string(),
                reason: format!(
                    "No audio track found in file (found {} tracks)",
                    format.tracks().len()
                ),
            })
        })?;

    let track_id = track.id;
    let audio_params = track
        .codec_params
        .as_ref()
        .and_then(CodecParameters::audio)
        .ok_or_else(|| {
            Error::Download(DownloadError::AudioExtractionFailed {
                title: title.to_string(),
                reason: format!("Audio track {track_id} has no codec parameters"),
            })
        })?;

    // symphonia only implements AAC-LC, so HE-AAC streams are decoded through
    // their AAC-LC core by rewriting the configuration the decoder is given.
    let mut decoder_params = audio_params.clone();
    if let Some(core) = audio_params
        .extra_data
        .as_deref()
        .and_then(aac_lc_core_config)
    {
        info!(
            "Track {} is HE-AAC - decoding its AAC-LC core at {} Hz (SBR high band dropped)",
            track_id, core.sample_rate
        );
        decoder_params
            .with_extra_data(core.config)
            .with_sample_rate(core.sample_rate);
    }

    // Create decoder for the audio track
    let mut decoder_codecs = symphonia::default::get_codecs()
        .make_audio_decoder(&decoder_params, &AudioDecoderOptions::default())
        .map_err(|e| {
            Error::Download(DownloadError::AudioExtractionFailed {
                title: title.to_string(),
                reason: format!("Failed to create audio decoder for track {track_id}: {e}"),
            })
        })?;

    // The decoder amends the parameters with what the AudioSpecificConfig says,
    // which is authoritative over the container's sample entry.
    let actual_params = decoder_codecs.codec_params();
    let sample_rate = actual_params.sample_rate.unwrap_or(44_100);
    let channels = actual_params.channels.as_ref().map_or(2, Channels::count);

    info!(
        "Found audio track {}: codec={:?}, {} Hz, {} channels - encoding to MP3",
        track_id, actual_params.codec, sample_rate, channels
    );

    // Create MP3 encoder
    let mut builder = LameBuilder::new().ok_or_else(|| {
        Error::Download(DownloadError::AudioExtractionFailed {
            title: title.to_string(),
            reason: "Failed to create MP3 encoder".to_string(),
        })
    })?;

    builder.set_sample_rate(sample_rate).map_err(|e| {
        Error::Download(DownloadError::AudioExtractionFailed {
            title: title.to_string(),
            reason: format!("Invalid sample rate for MP3: {e}"),
        })
    })?;

    // Basic MP3 players often only handle MPEG-1 rates (32/44.1/48 kHz), so have
    // LAME resample low-rate audio such as the 22.05 kHz HE-AAC core to 44.1 kHz.
    if sample_rate < 32_000 {
        builder
            .set_output_sample_rate(NonZeroU32::new(44_100))
            .map_err(|e| {
                Error::Download(DownloadError::AudioExtractionFailed {
                    title: title.to_string(),
                    reason: format!("Invalid output sample rate for MP3: {e}"),
                })
            })?;
    }

    let channels_u8: u8 = channels.try_into().map_err(|_err| {
        Error::Download(DownloadError::AudioExtractionFailed {
            title: title.to_string(),
            reason: format!("Channel count {channels} exceeds u8 max"),
        })
    })?;
    builder.set_num_channels(channels_u8).map_err(|e| {
        Error::Download(DownloadError::AudioExtractionFailed {
            title: title.to_string(),
            reason: format!("Invalid channel count for MP3: {e}"),
        })
    })?;

    builder
        .set_brate(mp3lame_encoder::Bitrate::Kbps128)
        .map_err(|e| {
            Error::Download(DownloadError::AudioExtractionFailed {
                title: title.to_string(),
                reason: format!("Invalid bitrate for MP3: {e}"),
            })
        })?;

    builder
        .set_quality(mp3lame_encoder::Quality::Best)
        .map_err(|e| {
            Error::Download(DownloadError::AudioExtractionFailed {
                title: title.to_string(),
                reason: format!("Invalid quality for MP3: {e}"),
            })
        })?;

    let mut encoder = builder.build().map_err(|e| {
        Error::Download(DownloadError::AudioExtractionFailed {
            title: title.to_string(),
            reason: format!("Failed to build MP3 encoder: {e}"),
        })
    })?;

    // Create output file with .mp3 extension
    let mp3_output_path = output_path.with_extension("mp3");
    let mut output_file = File::create(&mp3_output_path).map_err(|e| {
        Error::Download(DownloadError::AudioExtractionFailed {
            title: title.to_string(),
            reason: format!("Failed to create output file: {e}"),
        })
    })?;

    let mut total_samples = 0u64;
    let mut mp3_buffer = Vec::with_capacity(16384);
    let mut samples: Vec<i16> = Vec::new();

    // Decode and encode loop
    loop {
        let packet = match format.next_packet() {
            Ok(Some(p)) => p,
            // End of stream
            Ok(None) => break,
            Err(symphonia::core::errors::Error::IoError(ref e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(symphonia::core::errors::Error::ResetRequired) => {
                // Handle stream reset
                decoder_codecs.reset();
                continue;
            }
            Err(_) => break,
        };

        // Skip packets from other tracks (e.g., video track)
        if packet.track_id != track_id {
            continue;
        }

        let decoded_packet = match decoder_codecs.decode(&packet) {
            Ok(d) => d,
            Err(symphonia::core::errors::Error::DecodeError(_)) => continue,
            Err(_) => continue,
        };

        decoded_packet.copy_to_vec_interleaved(&mut samples);
        total_samples += samples.len() as u64;

        // Split into left/right for LAME
        let left: Vec<i16> = samples.iter().step_by(2).copied().collect();
        let right: Vec<i16> = if channels == 2 {
            samples.iter().skip(1).step_by(2).copied().collect()
        } else {
            left.clone()
        };

        let input = DualPcm {
            left: &left,
            right: &right,
        };

        mp3_buffer.clear();
        encoder.encode_to_vec(input, &mut mp3_buffer).map_err(|e| {
            Error::Download(DownloadError::AudioExtractionFailed {
                title: title.to_string(),
                reason: format!("MP3 encoding failed: {e}"),
            })
        })?;

        if !mp3_buffer.is_empty() {
            output_file.write_all(&mp3_buffer).map_err(|e| {
                Error::Download(DownloadError::AudioExtractionFailed {
                    title: title.to_string(),
                    reason: format!("Failed to write MP3 data: {e}"),
                })
            })?;
        }
    }

    // Flush encoder
    mp3_buffer.clear();
    encoder
        .flush_to_vec::<FlushNoGap>(&mut mp3_buffer)
        .map_err(|e| {
            Error::Download(DownloadError::AudioExtractionFailed {
                title: title.to_string(),
                reason: format!("Failed to flush MP3 encoder: {e}"),
            })
        })?;

    if !mp3_buffer.is_empty() {
        output_file.write_all(&mp3_buffer).map_err(|e| {
            Error::Download(DownloadError::AudioExtractionFailed {
                title: title.to_string(),
                reason: format!("Failed to write final MP3 data: {e}"),
            })
        })?;
    }

    info!(
        "Successfully re-encoded {} samples to MP3: {:?}",
        total_samples, mp3_output_path
    );

    Ok(mp3_output_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn he_aac_config_is_rewritten_to_its_lc_core() {
        // AOT 5 (SBR), 22050 Hz core (index 7), stereo, 44100 Hz extension, AOT 2 core.
        let he_aac = [0x2B, 0x92, 0x08, 0x00];
        let core = aac_lc_core_config(&he_aac).expect("HE-AAC should be recognised");
        assert_eq!(core.sample_rate, 22_050);
        // AOT 2, index 7, stereo.
        assert_eq!(&*core.config, &[0x13, 0x90]);
    }

    #[test]
    fn he_aac_v2_config_is_rewritten_to_its_mono_lc_core() {
        // AOT 29 (PS), 22050 Hz core (index 7), mono.
        let he_aac_v2 = [0xEB, 0x88, 0x00, 0x00];
        let core = aac_lc_core_config(&he_aac_v2).expect("HE-AACv2 should be recognised");
        assert_eq!(core.sample_rate, 22_050);
        assert_eq!(&*core.config, &[0x13, 0x88]);
    }

    #[test]
    fn plain_aac_lc_config_is_left_alone() {
        // AOT 2, 44100 Hz (index 4), stereo.
        assert!(aac_lc_core_config(&[0x12, 0x10]).is_none());
    }

    #[test]
    fn escaped_sample_rate_and_pce_channels_are_not_rewritten() {
        // AOT 5 with samplingFrequencyIndex 15 (explicit frequency follows).
        assert!(aac_lc_core_config(&[0x2F, 0x90, 0x00, 0x00, 0x00]).is_none());
        // AOT 5 with channelConfiguration 0 (layout defined by a PCE).
        assert!(aac_lc_core_config(&[0x2B, 0x80, 0x00]).is_none());
        // Too short to hold a configuration.
        assert!(aac_lc_core_config(&[0x2B]).is_none());
    }
}
