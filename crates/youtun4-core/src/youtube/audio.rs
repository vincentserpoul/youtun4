use std::path::Path;

use mp4::Mp4Reader;
use tracing::{info, warn};

// Audio re-encoding for HE-AAC compatibility
use mp3lame_encoder::{Builder as LameBuilder, DualPcm, FlushNoGap};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

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
) -> Result<()> {
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
    Ok(())
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

/// Re-encode AAC (including HE-AAC) to MP3 for compatibility with basic MP3 players.
///
/// This function uses symphonia to decode AAC audio and LAME to encode to MP3.
/// It's used when the source audio is HE-AAC (SBR/PS) which many portable
/// MP3 players don't support.
#[allow(
    clippy::too_many_lines,
    reason = "audio re-encoding requires sequential steps"
)]
fn reencode_aac_to_mp3(input_path: &Path, output_path: &Path, title: &str) -> Result<()> {
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

    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|e| {
            Error::Download(DownloadError::AudioExtractionFailed {
                title: title.to_string(),
                reason: format!("Failed to probe audio format: {e}"),
            })
        })?;

    let mut format = probed.format;

    // Find an audio track - MP4 files from YouTube have both video and audio tracks
    // We need to explicitly find a track with an audio codec
    let track = format
        .tracks()
        .iter()
        .find(|t| {
            // Check if this track has an audio codec type
            matches!(
                t.codec_params.codec,
                symphonia::core::codecs::CODEC_TYPE_AAC
                    | symphonia::core::codecs::CODEC_TYPE_MP3
                    | symphonia::core::codecs::CODEC_TYPE_FLAC
                    | symphonia::core::codecs::CODEC_TYPE_VORBIS
                    | symphonia::core::codecs::CODEC_TYPE_OPUS
            )
        })
        .or_else(|| {
            // Fallback: try to find any track with sample_rate (audio tracks have this)
            format
                .tracks()
                .iter()
                .find(|t| t.codec_params.sample_rate.is_some())
        })
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
    let sample_rate = track.codec_params.sample_rate.unwrap_or(44100);
    let channels = track
        .codec_params
        .channels
        .map_or(2, symphonia::core::audio::Channels::count);

    info!(
        "Found audio track {}: codec={:?}, {} Hz, {} channels - encoding to MP3",
        track_id, track.codec_params.codec, sample_rate, channels
    );

    // Create decoder for the audio track
    let mut decoder_codecs = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| {
            Error::Download(DownloadError::AudioExtractionFailed {
                title: title.to_string(),
                reason: format!("Failed to create audio decoder for track {track_id}: {e}"),
            })
        })?;

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

    // Decode and encode loop
    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
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
        if packet.track_id() != track_id {
            continue;
        }

        let decoded_packet = match decoder_codecs.decode(&packet) {
            Ok(d) => d,
            Err(symphonia::core::errors::Error::DecodeError(_)) => continue,
            Err(_) => continue,
        };

        let spec = *decoded_packet.spec();
        let mut sample_buf = SampleBuffer::<i16>::new(decoded_packet.capacity() as u64, spec);
        sample_buf.copy_interleaved_ref(decoded_packet);

        let samples = sample_buf.samples();
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

    Ok(())
}
