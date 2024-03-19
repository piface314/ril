//! Implements the font/text rasterizing and layout interface.

#![allow(clippy::cast_precision_loss, clippy::too_many_arguments)]

use crate::{Draw, Error::FontError, Image, OverlayMode, Pixel};

pub use fontdue::layout::{BlockAlign, HorizontalAlign, VerticalAlign, WrapStyle};
use fontdue::{
    layout::{CoordinateSystem, Layout, LayoutSettings},
    FontSettings,
};
use std::{fs::File, io::Read, ops::DerefMut, path::Path};

/// Represents a single font along with its alternatives used to render text.
/// Currently, this supports TrueType and OpenType fonts.
#[allow(clippy::doc_markdown)]
#[derive(Clone)]
pub struct Font {
    inner: fontdue::Font,
    settings: FontSettings,
}

impl Font {
    /// Opens the font from the given path.
    ///
    /// The optimal size is not the fixed size of the font - rather it is the size to optimize
    /// rasterizing the font for.
    ///
    /// Lower sizes will look worse but perform faster, while higher sizes will
    /// look better but perform slower. It is best to set this to the size that will likely be
    /// the most used.
    ///
    /// # Errors
    /// * Failed to load the font.
    pub fn open<P: AsRef<Path>>(path: P, optimal_size: f32) -> crate::Result<Self> {
        Self::from_reader(File::open(path)?, optimal_size)
    }

    /// Loads the font from the given byte slice. Useful for the `include_bytes!` macro.
    ///
    /// The optimal size is not the fixed size of the font - rather it is the size to optimize
    /// rasterizing the font for.
    ///
    /// Lower sizes will look worse but perform faster, while higher sizes will
    /// look better but perform slower. It is best to set this to the size that will likely be
    /// the most used.
    ///
    /// # Errors
    /// * Failed to load the font.
    pub fn from_bytes(bytes: &[u8], optimal_size: f32) -> crate::Result<Self> {
        let settings = FontSettings {
            scale: optimal_size,
            collection_index: 0,
            load_substitutions: true,
        };
        let inner = fontdue::Font::from_bytes(bytes, settings).map_err(FontError)?;

        Ok(Self { inner, settings })
    }

    /// Loads the font from the given byte reader. See [`from_bytes`] if you already have a byte
    /// slice - that is much more performant.
    ///
    /// The optimal size is not the fixed size of the font - rather it is the size to optimize
    /// rasterizing the font for.
    ///
    /// Lower sizes will look worse but perform faster, while higher sizes will
    /// look better but perform slower. It is best to set this to the size that will likely be
    /// the most used.
    ///
    /// # Errors
    /// * Failed to load the font.
    pub fn from_reader<R: Read>(mut buffer: R, optimal_size: f32) -> crate::Result<Self> {
        let settings = FontSettings {
            scale: optimal_size,
            collection_index: 0,
            load_substitutions: true,
        };
        let mut out = Vec::new();
        buffer.read_to_end(&mut out)?;

        let inner = fontdue::Font::from_bytes(out, settings).map_err(FontError)?;

        Ok(Self { inner, settings })
    }

    /// Returns a reference the [`fontdue::Font`] object associated with the font.
    /// It contains technical information about the font.
    #[must_use]
    pub const fn inner(&self) -> &fontdue::Font {
        &self.inner
    }

    /// Consumes this font and returns the [`fontdue::Font`] object associated with the font.
    /// It contains technical information about the font.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // no destructors
    pub fn into_inner(self) -> fontdue::Font {
        self.inner
    }

    /// Returns the optimal size, in pixels, of this font.
    ///
    /// The optimal size is not the fixed size of the font - rather it is the size to optimize
    /// rasterizing the font for.
    ///
    /// Lower sizes will look worse but perform faster, while higher sizes will
    /// look better but perform slower. It is best to set this to the size that will likely be
    /// the most used.
    #[must_use]
    pub const fn optimal_size(&self) -> f32 {
        self.settings.scale
    }
}

/// Represents where text is anchored horizontally.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum HorizontalAnchor {
    /// The x position is the left edge of the text. This is the default.
    Left,
    /// The x position is the center of the text. This also center-aligns the text.
    Center,
    /// The x position is the right edge of the text. This also right-aligns the text.
    Right,
}

impl Default for HorizontalAnchor {
    fn default() -> Self {
        Self::Left
    }
}

/// Represents where text is anchored vertically.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum VerticalAnchor {
    /// The y position is the top edge of the text. This is the default.
    Top,
    /// The y position is the center of the text.
    Center,
    /// The y position is the bottom edge of the text.
    Bottom,
    /// The y position is the text baseline of the first line of text.
    Baseline,
}

impl Default for VerticalAnchor {
    fn default() -> Self {
        Self::Top
    }
}

/// Represents additional data to render text or images inside a [`TextLayout`].
#[derive(Copy, Clone)]
enum SpanData<'a, P: Pixel> {
    /// Parameters for rendering text.
    Text(P, OverlayMode),
    /// Parameters for rendering an inline image.
    InlineImg(&'a Image<P>),
}

/// Represents a text segment that can be added to [`TextLayout`].
#[derive(Clone)]
pub struct TextSegment<'a, P: Pixel> {
    /// The content of the text segment.
    pub text: &'a str,
    /// The font to use to render the text.
    pub font: &'a Font,
    /// Font scale. By default, the font optimal scale is used.
    pub size: f32,
    /// The fill color the text will be in.
    pub fill: P,
    /// The overlay mode of the text. Note that anti-aliasing is still a bit funky with
    /// [`OverlayMode::Replace`], so it is best to use [`OverlayMode::Merge`] for this, which is
    /// the default.
    pub overlay: OverlayMode,
}

impl<'a, P: Pixel> TextSegment<'a, P> {
    /// Creates
    pub fn new(font: &'a Font, text: &'a str, fill: P) -> Self {
        Self {
            text,
            font,
            size: font.settings.scale,
            fill,
            overlay: OverlayMode::Merge,
        }
    }

    pub fn with_size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    pub fn with_ov(mut self, overlay: OverlayMode) -> Self {
        self.overlay = overlay;
        self
    }
}

#[derive(Copy, Clone)]
pub struct InlineImage<'a, P: Pixel> {
    pub image: &'a Image<P>,
    pub font: &'a Font,
    pub size: f32,
    pub align: BlockAlign,
}

impl<'a, P: Pixel> InlineImage<'a, P> {
    pub fn new(font: &'a Font, image: &'a Image<P>) -> Self {
        Self {
            image,
            font,
            size: font.settings.scale,
            align: BlockAlign::Middle,
        }
    }

    pub fn with_size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    pub fn with_align(mut self, align: BlockAlign) -> Self {
        self.align = align;
        self
    }
}

/// Represents a high-level text layout that can layout text segments, maybe with different fonts.
///
/// It can be used to layout text segments with different fonts and styles, and also inline images.
/// This also keeps track of font metrics, so it can be used to determine the width and height
/// of text before rendering it.
///
/// # Note
/// This is does not implement [`Clone`] and therefore it is not cloneable!
pub struct TextLayout<'a, P: Pixel> {
    layout: Layout<'a, SpanData<'a, P>>,
    settings: LayoutSettings,
    x_anchor: HorizontalAnchor,
    y_anchor: VerticalAnchor,
}

impl<'a, P: Pixel> TextLayout<'a, P> {
    /// Creates a new text layout with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self {
            layout: Layout::new(CoordinateSystem::PositiveYDown),
            settings: LayoutSettings::default(),
            x_anchor: HorizontalAnchor::default(),
            y_anchor: VerticalAnchor::default(),
        }
    }

    fn set_settings(&mut self, settings: &LayoutSettings) {
        self.layout.reset(settings);
        self.settings = *settings;
    }

    /// Sets all layout settings in one call.
    ///
    /// **This must be set before adding any text segments!**
    pub fn with_settings(mut self, settings: &LayoutSettings) -> Self {
        self.set_settings(settings);
        self
    }

    /// Sets the position of the text layout.
    ///
    /// **This must be set before adding any text segments!**
    #[must_use]
    pub fn with_position(mut self, x: u32, y: u32) -> Self {
        self.set_settings(&LayoutSettings {
            x: x as f32,
            y: y as f32,
            ..self.settings
        });
        self
    }

    /// Sets the anchor of the text layout.
    #[must_use]
    pub fn with_anchor(mut self, x_anchor: HorizontalAnchor, y_anchor: VerticalAnchor) -> Self {
        self.x_anchor = x_anchor;
        self.y_anchor = y_anchor;
        self
    }

    /// Sets the wrapping width of the text. This does not impact [`Self::width`] and [`Self::dimensions`].
    ///
    /// **This must be set before adding any text segments!**
    #[must_use]
    pub fn with_width(mut self, width: u32) -> Self {
        self.set_settings(&LayoutSettings {
            max_width: Some(width as f32),
            ..self.settings
        });
        self
    }

    /// Sets the maximum height of the text. This does not impact [`Self::height`] and [`Self::dimensions`].
    ///
    /// **This must be set before adding any text segments!**
    #[must_use]
    pub fn with_height(mut self, height: u32) -> Self {
        self.set_settings(&LayoutSettings {
            max_height: Some(height as f32),
            ..self.settings
        });
        self
    }

    /// Sets the wrapping style of the text. Make sure to also set the wrapping width using
    /// [`Self::with_width`] for wrapping to work.
    ///
    /// **This must be set before adding any text segments!**
    #[must_use]
    pub fn with_wrap(mut self, wrap: WrapStyle) -> Self {
        self.set_settings(&LayoutSettings {
            wrap_style: wrap,
            ..self.settings
        });
        self
    }

    /// Sets the horizontal text alignment.
    ///
    /// **This must be set before adding any text segments!**
    #[must_use]
    pub fn with_horizontal_align(mut self, align: HorizontalAlign) -> Self {
        self.set_settings(&LayoutSettings {
            horizontal_align: align,
            ..self.settings
        });
        self
    }

    /// Sets the vertical text alignment.
    ///
    /// **This must be set before adding any text segments!**
    #[must_use]
    pub fn with_vertical_align(mut self, align: VerticalAlign) -> Self {
        self.set_settings(&LayoutSettings {
            vertical_align: align,
            ..self.settings
        });
        self
    }

    /// Sets the height of each line as a multiplier of the default.
    ///
    /// **This must be set before adding any text segments!**
    #[must_use]
    pub fn with_line_height(mut self, line_height: f32) -> Self {
        self.set_settings(&LayoutSettings {
            line_height,
            ..self.settings
        });
        self
    }

    /// Sets both horizontal and vertical anchor and alignment of the text to be centered.
    /// This makes the position of the text be the center as opposed to the top-left corner.
    ///
    /// **This must be set before adding any text segments!**
    #[must_use]
    pub fn centered(mut self) -> Self {
        self.set_settings(&LayoutSettings {
            horizontal_align: HorizontalAlign::Center,
            vertical_align: VerticalAlign::Middle,
            ..self.settings
        });
        self.x_anchor = HorizontalAnchor::Center;
        self.y_anchor = VerticalAnchor::Center;
        self
    }

    /// Adds a text segment to the text layout.
    pub fn push_text(&mut self, segment: &TextSegment<'a, P>) {
        let user_data = SpanData::Text(segment.fill, segment.overlay);
        self.layout
            .append(&fontdue::layout::Span::text_with_user_data(
                segment.text,
                segment.size,
                segment.font.inner(),
                user_data,
            ));
    }

    /// Takes this text layout and returns it with the given text segment added to the text layout.
    /// Useful for method chaining.
    #[must_use]
    pub fn with_text(mut self, segment: &TextSegment<'a, P>) -> Self {
        self.push_text(segment);
        self
    }

    /// Adds basic text to the text layout. This is a convenience method that creates a [`TextSegment`]
    /// with the given font, text, and fill and adds it to the text layout.
    ///
    /// The size of the text is determined by the font's optimal size.
    ///
    /// # Note
    /// The overlay mode is set to [`OverlayMode::Merge`] and not the image's overlay mode, since
    /// anti-aliasing is funky with the replace overlay mode.
    pub fn push_basic_text(&mut self, font: &'a Font, text: &'a str, fill: P) {
        self.push_text(&TextSegment::new(font, text, fill));
    }

    /// Takes this text layout and returns it with the given basic text added to the text layout.
    /// Useful for method chaining.
    ///
    /// # Note
    /// The overlay mode is set to [`OverlayMode::Merge`] and not the image's overlay mode, since
    /// anti-aliasing is funky with the replace overlay mode.
    ///
    /// # See Also
    /// * [`push_basic_text`][TextLayout::push_basic_text]
    #[must_use]
    pub fn with_basic_text(mut self, font: &'a Font, text: &'a str, fill: P) -> Self {
        self.push_basic_text(font, text, fill);
        self
    }

    /// Adds an inline to the text layout.
    pub fn push_image(&mut self, img: &InlineImage<'a, P>) {
        let user_data = SpanData::InlineImg(img.image);
        let (w, h) = img.image.dimensions();
        self.layout.append(&fontdue::layout::Span::block(
            w as usize,
            h as usize,
            img.align,
            img.size,
            img.font.inner(),
            user_data,
        ));
    }

    /// Takes this text layout and returns it with the given inline image added to the text layout.
    /// Useful for method chaining.
    #[must_use]
    pub fn with_image(mut self, image: &InlineImage<'a, P>) -> Self {
        self.push_image(image);
        self
    }

    /// Returns the width of the text. This is a slightly expensive operation and is not a simple
    /// getter.
    ///
    /// If you want both width and height, use [`dimensions`][TextLayout::dimensions].
    #[must_use]
    pub fn width(&self) -> u32 {
        let mut width = 0;

        if let Some(lines) = self.layout.lines() {
            let glyphs = self.layout.glyphs();
            for line in lines {
                let x = self.settings.x as u32;

                for glyph in glyphs[line.glyph_start..=line.glyph_end].iter().rev() {
                    if glyph.char_data.is_whitespace() {
                        continue;
                    }

                    let right = glyph.x + glyph.width as f32;
                    let line_width = (right - x as f32).ceil() as u32;
                    width = width.max(line_width);

                    break;
                }
            }
        }

        width
    }

    /// Returns the height of the text.
    ///
    /// If you want both width and height, use [`dimensions`][TextLayout::dimensions].
    #[must_use]
    pub fn height(&self) -> u32 {
        self.layout.height() as u32
    }

    /// Returns the width and height of the text. This is a slightly expensive operation and should
    /// be used sparingly - it is not a simple getter.
    #[must_use]
    pub fn dimensions(&self) -> (u32, u32) {
        (self.width(), self.height())
    }

    /// Returns the bounding box of the text. Left and top bounds are inclusive; right and bottom
    /// bounds are exclusive.
    #[must_use]
    pub fn bounding_box(&self) -> (u32, u32, u32, u32) {
        let (width, height) = self.dimensions();

        let ox = match self.x_anchor {
            HorizontalAnchor::Left => 0.0,
            HorizontalAnchor::Center => width as f32 / -2.0,
            HorizontalAnchor::Right => -(width as f32),
        };
        let oy = match self.y_anchor {
            VerticalAnchor::Top => 0.0,
            VerticalAnchor::Baseline => {
                if let Some(lines) = self.layout.lines() {
                    lines.last().map(|line| -line.max_ascent).unwrap_or(0.0)
                } else {
                    0.0
                }
            },
            VerticalAnchor::Center => height as f32 / -2.0,
            VerticalAnchor::Bottom => -(height as f32),
        };

        let x = (self.settings.x + ox) as u32;
        let y = (self.settings.y + oy) as u32;

        (x, y, x + width, y + height)
    }

    fn offsets(&self) -> (f32, f32) {
        let ox = match self.x_anchor {
            HorizontalAnchor::Left => 0.0,
            HorizontalAnchor::Center => {
                self.settings
                    .max_width
                    .unwrap_or_else(|| self.width() as f32)
                    * -0.5
            }
            HorizontalAnchor::Right => {
                self.settings
                    .max_width
                    .unwrap_or_else(|| self.width() as f32)
                    * -1.0
            }
        };
        let oy = match self.y_anchor {
            VerticalAnchor::Top => 0.0,
            VerticalAnchor::Baseline => {
                if let Some(lines) = self.layout.lines() {
                    lines.last().map(|line| -line.max_ascent).unwrap_or(0.0)
                } else {
                    0.0
                }
            }
            VerticalAnchor::Center => {
                self.settings
                    .max_height
                    .unwrap_or_else(|| self.layout.height())
                    * -0.5
            }
            VerticalAnchor::Bottom => {
                self.settings
                    .max_height
                    .unwrap_or_else(|| self.layout.height())
                    * -1.0
            }
        };
        (ox, oy)
    }
}

impl<'a, P: Pixel> Draw<P> for TextLayout<'a, P> {
    fn draw<I: DerefMut<Target = Image<P>>>(&self, mut image: I) {
        if let Some(lines) = self.layout.lines() {
            let image = &mut *image;
            let glyphs = self.layout.glyphs();
            let (ox, oy) = self.offsets();
            for line in lines.iter() {
                for glyph in &glyphs[line.glyph_start..=line.glyph_end] {
                    let font = glyph.font;
                    let x = (glyph.x + ox) as i32;
                    let y = (glyph.y + oy) as i32;
                    match glyph.user_data {
                        SpanData::Text(fill, overlay) => {
                            let (metrics, bitmap) = font.rasterize_config(glyph.key.unwrap());
                            if metrics.width == 0
                                || glyph.char_data.is_whitespace()
                                || metrics.height == 0
                            {
                                continue;
                            }

                            for (row, y) in bitmap.chunks_exact(metrics.width).zip(y..) {
                                for (value, x) in row.iter().zip(x..) {
                                    let (x, y) = if x < 0 || y < 0 {
                                        continue;
                                    } else {
                                        (x as u32, y as u32)
                                    };

                                    let value = *value;
                                    if value == 0 {
                                        continue;
                                    }

                                    if let Some(pixel) = image.get_pixel(x, y) {
                                        *image.pixel_mut(x, y) =
                                            pixel.overlay_with_alpha(fill, overlay, value);
                                    }
                                }
                            }
                        }
                        SpanData::InlineImg(other) => {
                            let (x, y) = if x < 0 || y < 0 {
                                continue;
                            } else {
                                (x as u32, y as u32)
                            };
                            image.paste(x, y, other);
                        }
                    }
                }
            }
        }
    }
}
