use super::{ColorType, PixelData};
use crate::{
    encode::{Decoder, Encoder},
    DisposalMethod,
    Frame,
    Image,
    ImageFormat,
    ImageSequence,
    LoopCount,
    Pixel,
};

pub use png::{AdaptiveFilterType, Compression, FilterType};
use std::{
    io::{Read, Write},
    marker::PhantomData,
    num::NonZeroU32,
    time::Duration,
};

impl From<png::ColorType> for ColorType {
    fn from(value: png::ColorType) -> Self {
        use png::ColorType::*;

        match value {
            Grayscale => Self::L,
            GrayscaleAlpha => Self::LA,
            Rgb => Self::Rgb,
            Rgba => Self::Rgba,
            Indexed => Self::Palette,
        }
    }
}

fn get_png_color_type(src: ColorType) -> png::ColorType {
    use png::ColorType::*;

    match src {
        ColorType::L => Grayscale,
        ColorType::LA => GrayscaleAlpha,
        ColorType::Rgb => Rgb,
        ColorType::Rgba => Rgba,
        ColorType::Palette => Indexed,
    }
}

/// A PNG encoder interface around [`png::Encoder`].
pub struct PngEncoder {
    /// The adaptive filter type to use.
    pub adaptive_filter: AdaptiveFilterType,
    /// The filter type to use.
    pub filter: FilterType,
    /// The compression to use.
    pub compression: Compression,
}

impl PngEncoder {
    /// Creates a new encoder with the default settings.
    #[must_use]
    pub fn new() -> Self {
        Self {
            adaptive_filter: AdaptiveFilterType::NonAdaptive,
            filter: FilterType::NoFilter,
            compression: Compression::Default,
        }
    }

    /// Sets the adaptive filter type to use.
    pub fn with_adaptive_filter(mut self, value: AdaptiveFilterType) -> Self {
        self.adaptive_filter = value;
        self
    }

    /// Sets the filter type to use.
    pub fn with_filter(mut self, value: FilterType) -> Self {
        self.filter = value;
        self
    }

    /// Sets the compression level to use.
    pub fn with_compression(mut self, value: Compression) -> Self {
        self.compression = value;
        self
    }

    fn prepare<P: Pixel, W: Write>(&mut self, width: u32, height: u32, sample: &P, dest: &mut W) -> png::Encoder<W> {
        let mut encoder = png::Encoder::new(dest, width, height);

        encoder.set_adaptive_filter(self.adaptive_filter);
        encoder.set_filter(self.filter);
        encoder.set_compression(self.compression);

        let (color_type, bit_depth) = sample.as_pixel_data().type_data();

        encoder.set_color(get_png_color_type(color_type));
        encoder.set_depth(png::BitDepth::from_u8(bit_depth).unwrap());

        encoder
    }
}

impl Encoder for PngEncoder {
    fn encode<P: Pixel>(&mut self, image: &Image<P>, dest: &mut impl Write) -> crate::Result<()> {
        let data = image
            .data
            .iter()
            .flat_map(|pixel| pixel.as_pixel_data().data())
            .collect::<Vec<_>>();

        let mut encoder = self.prepare(image.width(), image.height(), &image.data[0], dest);
        let mut writer = encoder.write_header()?;
        writer.write_image_data(&data)?;
        writer.finish()?;

        Ok(())
    }

    fn encode_sequence<P: Pixel>(&mut self, sequence: &ImageSequence<P>, dest: &mut impl Write) -> crate::Result<()> {
        let sample = &sequence.first_frame().image().data[0];

        let mut encoder = self.prepare(sequence.width(), sequence.height(), sample, dest);
        encoder.set_animated(sequence.len() as u32, sequence.loop_count().count_or_zero())?;

        let mut writer = encoder.write_header()?;

        for frame in sequence.iter() {
            let data = frame
                .image()
                .data
                .iter()
                .flat_map(|pixel| pixel.as_pixel_data().data())
                .collect::<Vec<_>>();

            writer.set_frame_delay(frame.delay().as_millis() as u16, 1000)?;
            writer.set_dispose_op(match frame.disposal() {
                DisposalMethod::None => png::DisposeOp::None,
                DisposalMethod::Background => png::DisposeOp::Background,
                DisposalMethod::Previous => png::DisposeOp::Previous,
            })?;
            writer.write_image_data(&data)?;
        }

        writer.finish()?;
        Ok(())
    }
}

/// A PNG decoder interface around [`png::Decoder`].
pub struct PngDecoder;

impl PngDecoder {
    /// Creates a new decoder with the default settings.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    fn prepare<R: Read>(&mut self, stream: R) -> crate::Result<png::Reader<R>> {
        let decoder = png::Decoder::new(stream);
        decoder.read_info().map_err(Into::into)
    }
}

impl Decoder for PngDecoder {
    fn decode<P: Pixel>(&mut self, stream: impl Read) -> crate::Result<Image<P>> {
        let mut reader = self.prepare(stream)?;

        let info = reader.info();
        let color_type: ColorType = info.color_type.into();
        let bit_depth = info.bit_depth as u8;

        // Here we are decoding a single image, so only capture the first frame:
        let buffer = &mut vec![0; reader.output_buffer_size()];
        reader.next_frame(buffer)?;

        let data = buffer
            .chunks_exact(info.bytes_per_pixel())
            .map(|chunk| {
                PixelData::from_raw(color_type, bit_depth, chunk).and_then(P::from_pixel_data)
            })
            .collect::<crate::Result<Vec<_>>>()?;

        Ok(Image {
            width: NonZeroU32::new(info.width).unwrap(),
            height: NonZeroU32::new(info.height).unwrap(),
            data,
            format: ImageFormat::Png,
            overlay: Default::default(),
            background: Default::default(),
        })
    }

    fn decode_sequence<P: Pixel, I>(&mut self, stream: impl Read) -> crate::Result<I>
    where
        I: Iterator<Item = Frame<P>>,
    {
        let reader = self.prepare(stream)?;

        Ok(ApngFrameIterator {
            seq: 0,
            reader,
            _marker: PhantomData,
        })
    }
}

pub struct ApngFrameIterator<P: Pixel, R: Read> {
    seq: u32,
    reader: png::Reader<R>,
    _marker: PhantomData<P>,
}

impl<P: Pixel, R: Read> ApngFrameIterator<P, R> {
    fn info(&self) -> &png::Info {
        &self.reader.info()
    }

    fn next_frame(&mut self) -> crate::Result<(&[u8], png::OutputInfo)> {
        let buffer = &mut vec![0; self.reader.output_buffer_size()];
        let info = self.reader.next_frame(buffer)?;

        Ok((buffer, info))
    }

    pub fn len(&self) -> u32 {
        self.info().animation_control.map(|a| a.num_frames).unwrap_or(1)
    }

    pub fn loop_count(&self) -> LoopCount {
        match self.info().animation_control.map(|a| a.loop_count) {
            Some(0) => LoopCount::Infinite,
            Some(n) => LoopCount::Exactly(n),
            None => LoopCount::Infinite,
        }
    }

    pub fn into_sequence(self) -> ImageSequence<P> {
        ImageSequence::from_frames(self.collect()).with_loop_count(self.loop_count())
    }
}

impl<P: Pixel, R: Read> Iterator for ApngFrameIterator<P, R> {
    type Item = crate::Result<Frame<P>>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.seq >= self.len() {
            return None;
        }

        let (frame, output_info) = self.next_frame()?;

        let info = self.info();
        let color_type: ColorType = info.color_type.into();
        let bit_depth = info.bit_depth as u8;

        let data = frame
            .chunks_exact(info.bytes_per_pixel())
            .map(|chunk| {
                PixelData::from_raw(color_type, bit_depth, chunk).and_then(P::from_pixel_data)
            })
            .collect::<crate::Result<Vec<_>>>()?;

        let inner = Image {
            width: NonZeroU32::new(output_info.width).unwrap(),
            height: NonZeroU32::new(output_info.height).unwrap(),
            data,
            format: ImageFormat::Png,
            overlay: Default::default(),
            background: Default::default(),
        };

        self.seq += 1;

        Some(Ok(
            Frame::from_image(inner)
                .with_delay(info.frame_control
                    .map(|f| Duration::from_secs_f64(f.delay_num as f64 / f.delay_den as f64))
                    .unwrap_or_else(Duration::default)
                )
                .with_disposal(info.frame_control
                    .map(|f| match f.dispose_op {
                        png::DisposeOp::None => DisposalMethod::None,
                        png::DisposeOp::Background => DisposalMethod::Background,
                        png::DisposeOp::Previous => DisposalMethod::Previous,
                    })
                    .unwrap_or_else(DisposalMethod::default)
                )
        ))
    }
}
