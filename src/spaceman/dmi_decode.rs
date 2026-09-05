use super::DmiError;
use crate::limits::ServerLimits;
use dmm_tools::dmi::{IconFile, Image, Rgba8};
use std::borrow::Cow;

fn invalid(message: &str) -> DmiError {
    DmiError::Invalid(message.into())
}

pub(super) fn dimensions(
    bytes: &[u8],
    limits: &ServerLimits,
) -> Result<(u32, u32, usize), DmiError> {
    if bytes.len() < 33 || &bytes[..8] != b"\x89PNG\r\n\x1a\n" || &bytes[8..16] != b"\0\0\0\rIHDR" {
        return Err(invalid("missing PNG image header"));
    }
    let width = u32::from_be_bytes(bytes[16..20].try_into().unwrap());
    let height = u32::from_be_bytes(bytes[20..24].try_into().unwrap());
    if width == 0 || height == 0 {
        return Err(invalid("empty PNG dimensions"));
    }
    let pixels = u64::from(width) * u64::from(height);
    if pixels > limits.max_dmi_decoded_pixels {
        return Err(DmiError::Limit("max_dmi_decoded_pixels".into()));
    }
    let decoded = pixels
        .checked_mul(4)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| DmiError::Limit("max_dmi_decoded_pixels".into()))?;
    Ok((width, height, decoded))
}

fn description<'a>(bytes: &'a [u8], limit: usize) -> Result<Option<Cow<'a, str>>, DmiError> {
    let mut offset = 8_usize;
    while offset < bytes.len() {
        let header = bytes
            .get(offset..offset.saturating_add(8))
            .ok_or_else(|| invalid("truncated PNG chunk"))?;
        let length = u32::from_be_bytes(header[..4].try_into().unwrap()) as usize;
        let end = offset
            .checked_add(12)
            .and_then(|value| value.checked_add(length))
            .filter(|&value| value <= bytes.len())
            .ok_or_else(|| invalid("truncated PNG chunk"))?;
        let kind = &header[4..];
        let data = &bytes[offset + 8..end - 4];
        if kind == b"tEXt" || kind == b"zTXt" {
            if let Some(separator) = data.iter().position(|byte| *byte == 0) {
                let (key, value) = (&data[..separator], &data[separator + 1..]);
                if key == b"Description" {
                    let text = if kind == b"tEXt" {
                        if value.len() > limit {
                            return Err(DmiError::Limit("max_dmi_metadata_bytes".into()));
                        }
                        Cow::Borrowed(
                            std::str::from_utf8(value)
                                .map_err(|_| invalid("DMI metadata is not UTF-8"))?,
                        )
                    } else {
                        if value.first() != Some(&0) {
                            return Err(invalid("unsupported DMI text compression"));
                        }
                        let decoded = fdeflate::decompress_to_vec_bounded(&value[1..], limit)
                            .map_err(|error| match error {
                                fdeflate::BoundedDecompressionError::OutputTooLarge { .. } => {
                                    DmiError::Limit("max_dmi_metadata_bytes".into())
                                }
                                _ => invalid("invalid compressed DMI metadata"),
                            })?;
                        Cow::Owned(
                            String::from_utf8(decoded)
                                .map_err(|_| invalid("DMI metadata is not UTF-8"))?,
                        )
                    };
                    return Ok(Some(text));
                }
            }
        }
        if kind == b"IEND" {
            break;
        }
        offset = end;
    }
    Ok(None)
}

fn bounded_image_stream(bytes: &[u8], limits: &ServerLimits) -> Result<(), DmiError> {
    let (width, height, _) = dimensions(bytes, limits)?;
    let channels = match bytes[25] {
        0 | 3 => 1_u64,
        2 => 3,
        4 => 2,
        6 => 4,
        _ => return Err(invalid("unsupported PNG color type")),
    };
    let bits = channels * u64::from(bytes[24]);
    let passes: &[(u64, u64, u64, u64)] = if bytes[28] == 0 {
        &[(0, 0, 1, 1)]
    } else {
        &[
            (0, 0, 8, 8),
            (4, 0, 8, 8),
            (0, 4, 4, 8),
            (2, 0, 4, 4),
            (0, 2, 2, 4),
            (1, 0, 2, 2),
            (0, 1, 1, 2),
        ]
    };
    let mut expected = 0_u64;
    for &(x, y, step_x, step_y) in passes {
        let pass_width = u64::from(width).saturating_sub(x).div_ceil(step_x);
        let pass_height = u64::from(height).saturating_sub(y).div_ceil(step_y);
        if pass_width != 0 && pass_height != 0 {
            expected = expected
                .checked_add(
                    (pass_width * bits)
                        .div_ceil(8)
                        .saturating_add(1)
                        .saturating_mul(pass_height),
                )
                .ok_or_else(|| DmiError::Limit("max_dmi_decoder_bytes".into()))?;
        }
    }
    let expected = usize::try_from(expected)
        .ok()
        .filter(|&value| value <= limits.max_dmi_decoder_bytes)
        .ok_or_else(|| DmiError::Limit("max_dmi_decoder_bytes".into()))?;
    let mut offset = 8_usize;
    let mut encoded_len = 0_usize;
    while offset < bytes.len() {
        let header = bytes
            .get(offset..offset.saturating_add(8))
            .ok_or_else(|| invalid("truncated PNG chunk"))?;
        let length = u32::from_be_bytes(header[..4].try_into().unwrap()) as usize;
        let end = offset
            .checked_add(12)
            .and_then(|value| value.checked_add(length))
            .filter(|&value| value <= bytes.len())
            .ok_or_else(|| invalid("truncated PNG chunk"))?;
        match &header[4..] {
            b"acTL" | b"fcTL" | b"fdAT" => {
                return Err(invalid("animated PNG is not supported as DMI"))
            }
            b"IDAT" => encoded_len += length,
            b"IEND" => break,
            _ => {}
        }
        offset = end;
    }
    let mut encoded = Vec::with_capacity(encoded_len);
    offset = 8;
    while offset < bytes.len() {
        let length = u32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
        let kind = &bytes[offset + 4..offset + 8];
        if kind == b"IDAT" {
            encoded.extend_from_slice(&bytes[offset + 8..offset + 8 + length]);
        }
        if kind == b"IEND" {
            break;
        }
        offset += length + 12;
    }
    // png 0.17's internal IDAT output ceiling is an optimization, not a hard
    // limit. Validate expansion once within the declared filtered-image budget,
    // then release this temporary before the pixel decoder allocates its output.
    let filtered =
        fdeflate::decompress_to_vec_bounded(&encoded, expected).map_err(|error| match error {
            fdeflate::BoundedDecompressionError::OutputTooLarge { .. } => {
                DmiError::Limit("max_dmi_decoded_pixels".into())
            }
            _ => invalid("invalid PNG image compression"),
        })?;
    if filtered.len() != expected {
        return Err(invalid("PNG image data does not match dimensions"));
    }
    Ok(())
}

fn bounded_frames(
    dirs: usize,
    frames: usize,
    total: usize,
    limits: &ServerLimits,
) -> Result<usize, DmiError> {
    dirs.checked_mul(frames)
        .and_then(|value| total.checked_add(value))
        .filter(|&value| value <= limits.max_dmi_frames && value <= u32::MAX as usize)
        .ok_or_else(|| DmiError::Limit("max_dmi_frames".into()))
}

// Validate the allocation counts and unchecked arithmetic before entering the
// pinned metadata parser. Its semantic representation remains authoritative.
fn preflight_metadata(text: &str, limits: &ServerLimits) -> Result<(), DmiError> {
    if text.is_empty() {
        return Ok(());
    }
    let mut lines = text.lines();
    if lines.next() != Some("# BEGIN DMI") || lines.next() != Some("version = 4.0") {
        return Err(invalid("invalid DMI metadata header"));
    }
    let mut states = 0_usize;
    let mut total = 0;
    let mut state: Option<(usize, usize, bool, bool)> = None;
    for line in lines {
        if line.starts_with("# END DMI") {
            break;
        }
        let (key, value) = line
            .trim()
            .split_once(" = ")
            .ok_or_else(|| invalid("malformed DMI metadata line"))?;
        match key {
            "width" | "height" => {
                if value
                    .parse::<u32>()
                    .ok()
                    .filter(|&value| value > 0)
                    .is_none()
                {
                    return Err(invalid("invalid DMI cell dimensions"));
                }
            }
            "state" => {
                if let Some((dirs, frames, _, _)) = state {
                    total = bounded_frames(dirs, frames, total, limits)?;
                }
                states += 1;
                if states > limits.max_dmi_states {
                    return Err(DmiError::Limit("max_dmi_states".into()));
                }
                if value.len() < 2 || !value.starts_with('"') || !value.ends_with('"') {
                    return Err(invalid("malformed DMI state name"));
                }
                state = Some((1, 1, false, false));
            }
            "dirs" | "frames" | "delay" | "loop" | "rewind" | "movement" => {
                let (dirs, frames, has_frames, has_delay) = state
                    .as_mut()
                    .ok_or_else(|| invalid("DMI state property without state"))?;
                match key {
                    "dirs" => {
                        *dirs = value
                            .parse()
                            .ok()
                            .filter(|value| matches!(value, 1 | 4 | 8))
                            .ok_or_else(|| invalid("invalid DMI direction count"))?;
                    }
                    "frames" => {
                        if *has_frames || *has_delay {
                            return Err(invalid("duplicate DMI frames property"));
                        }
                        *frames = value
                            .parse()
                            .map_err(|_| invalid("invalid DMI frame count"))?;
                        *has_frames = true;
                    }
                    "delay" => {
                        if *has_delay {
                            return Err(invalid("duplicate DMI delay property"));
                        }
                        let count = value.split(',').count();
                        if count > limits.max_dmi_frames {
                            return Err(DmiError::Limit("max_dmi_frames".into()));
                        }
                        for delay in value.split(',') {
                            if delay
                                .parse::<f32>()
                                .ok()
                                .filter(|value| value.is_finite())
                                .is_none()
                            {
                                return Err(invalid("invalid DMI delay"));
                            }
                        }
                        if !*has_frames {
                            *frames = count;
                        }
                        *has_delay = true;
                    }
                    "loop" => {
                        value
                            .parse::<u32>()
                            .map_err(|_| invalid("invalid DMI loop count"))?;
                    }
                    _ => {
                        value
                            .parse::<u8>()
                            .map_err(|_| invalid("invalid DMI state flag"))?;
                    }
                }
                bounded_frames(*dirs, *frames, total, limits)?;
            }
            "hotspot" => {}
            _ => return Err(invalid("unknown DMI metadata property")),
        }
    }
    if let Some((dirs, frames, _, _)) = state {
        bounded_frames(dirs, frames, total, limits)?;
    }
    Ok(())
}

pub(super) fn validate_metadata(
    metadata: &dmi::Metadata,
    width: u32,
    height: u32,
    limits: &ServerLimits,
) -> Result<(), DmiError> {
    if metadata.width == 0
        || metadata.height == 0
        || metadata.width > width
        || metadata.height > height
    {
        return Err(invalid("DMI cell dimensions are outside image"));
    }
    if metadata.states.len() > limits.max_dmi_states {
        return Err(DmiError::Limit("max_dmi_states".into()));
    }
    let mut frames = 0;
    for state in &metadata.states {
        frames = bounded_frames(state.dirs.count(), state.frames.count(), frames, limits)?;
    }
    let cells = u64::from(width / metadata.width) * u64::from(height / metadata.height);
    if frames as u64 > cells {
        return Err(invalid("DMI frames exceed image cells"));
    }
    Ok(())
}

pub(super) fn decode(
    bytes: &[u8],
    limits: &ServerLimits,
    before_decode: impl FnOnce(),
) -> Result<(IconFile, usize), DmiError> {
    let (width, height, _) = dimensions(bytes, limits)?;
    let description = description(bytes, limits.max_dmi_metadata_bytes)?;
    let metadata_bytes = description.as_ref().map_or(0, |text| text.len());
    let metadata = if let Some(text) = description {
        preflight_metadata(&text, limits)?;
        std::panic::catch_unwind(|| dmi::Metadata::meta_from_str(&text))
            .map_err(|_| invalid("malformed DMI metadata"))??
    } else {
        dmi::Metadata {
            width,
            height,
            states: Vec::new(),
            state_names: Default::default(),
        }
    };
    validate_metadata(&metadata, width, height, limits)?;
    let mut options = png::DecodeOptions::default();
    options.set_skip_ancillary_crc_failures(false);
    let mut decoder = png::Decoder::new_with_options(bytes, options);
    decoder.set_limits(png::Limits {
        bytes: limits.max_dmi_decoder_bytes,
    });
    decoder.set_ignore_text_chunk(true);
    decoder.set_ignore_iccp_chunk(true);
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(png_error)?;
    bounded_image_stream(bytes, limits)?;
    before_decode();
    let mut pixels = vec![0_u8; reader.output_buffer_size()];
    let output = reader.next_frame(&mut pixels).map_err(png_error)?;
    let mut image = Image::new_rgba(width, height);
    let channels = output.color_type.samples();
    for (destination, pixel) in image
        .data
        .as_slice_mut()
        .unwrap()
        .iter_mut()
        .zip(pixels[..output.buffer_size()].chunks_exact(channels))
    {
        *destination = match output.color_type {
            png::ColorType::Rgba => Rgba8::new(pixel[0], pixel[1], pixel[2], pixel[3]),
            png::ColorType::Rgb => Rgba8::new(pixel[0], pixel[1], pixel[2], 255),
            png::ColorType::Grayscale => Rgba8::new(pixel[0], pixel[0], pixel[0], 255),
            png::ColorType::GrayscaleAlpha => Rgba8::new(pixel[0], pixel[0], pixel[0], pixel[1]),
            png::ColorType::Indexed => return Err(invalid("PNG palette was not expanded")),
        };
    }
    reader.finish().map_err(png_error)?;
    Ok((IconFile { metadata, image }, metadata_bytes))
}

fn png_error(error: png::DecodingError) -> DmiError {
    match error {
        png::DecodingError::LimitsExceeded => DmiError::Limit("max_dmi_decoder_bytes".into()),
        _ => DmiError::Invalid(error.to_string()),
    }
}
