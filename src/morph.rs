//! Implements morphological transformations e.g. dilate and erode

use std::ops::DerefMut;

use crate::pixel::{BitPixel, Pixel, L};
use crate::{Banded, Draw, Image, Rgba};

/// Useful kernel shapes for morphology operations
#[derive(Copy, Clone, Debug)]
pub enum KernelShape {
    /// Rectangular kernel, completely filled
    Rect,
    /// Cross shaped kernel, a pair of centered horizontal and vertical lines
    Cross,
    /// Ellipse shaped kernel, without anti alias
    Ellipse,
    /// Ellipse shaped kernel, with anti alias
    EllipseAa,
}

pub struct KernelImage {
    data: Vec<f32>,
    width: u32,
    height: u32,
}

impl Into<Image<L>> for KernelImage {
    fn into(self) -> Image<L> {
        let pixels: Vec<L> = self
            .data
            .into_iter()
            .map(|p| L((p * u8::MAX as f32) as u8))
            .collect();
        Image::from_pixels(self.width, pixels)
    }
}

impl Into<Image<BitPixel>> for KernelImage {
    fn into(self) -> Image<BitPixel> {
        let pixels: Vec<BitPixel> = self.data.into_iter().map(|p| BitPixel(p > 0.0)).collect();
        Image::from_pixels(self.width, pixels)
    }
}

impl KernelImage {
    #[inline]
    #[must_use]
    const fn resolve_coordinate(&self, x: u32, y: u32) -> usize {
        (y * self.width + x) as usize
    }

    /// Returns a reference of the pixel at the given coordinates, but only if it exists.
    #[inline]
    #[must_use]
    pub fn get_pixel(&self, x: u32, y: u32) -> Option<&f32> {
        self.data.get(self.resolve_coordinate(x, y))
    }

    /// Returns a reference of the pixel at the given coordinates.
    #[inline]
    #[must_use]
    pub fn pixel(&self, x: u32, y: u32) -> &f32 {
        &self.data[self.resolve_coordinate(x, y)]
    }

    /// Returns a mutable reference to the pixel at the given coordinates.
    #[inline]
    pub fn pixel_mut(&mut self, x: u32, y: u32) -> &mut f32 {
        let pos = self.resolve_coordinate(x, y);
        &mut self.data[pos]
    }

    /// Creates an empty kernel image.
    #[must_use]
    pub fn new(width: u32, height: u32) -> Self {
        KernelImage {
            data: vec![0.0; (width * height) as usize],
            width,
            height,
        }
    }

    /// Creates a kernel image from the specified shape.
    #[must_use]
    pub fn from_shape(shape: KernelShape, width: u32, height: u32) -> Self {
        let mut kernel = Self::new(width, height);
        match shape {
            KernelShape::Rect => {
                for i in 0..kernel.data.len() {
                    kernel.data[i] = 1.0;
                }
            }
            KernelShape::Cross => {
                let w2 = width / 2;
                let h2 = height / 2;
                for y in 0..height {
                    *kernel.pixel_mut(w2, y) = 1.0;
                }
                for x in 0..width {
                    *kernel.pixel_mut(x, h2) = 1.0;
                }
            }
            KernelShape::Ellipse => kernel.draw_ellipse(false),
            KernelShape::EllipseAa => kernel.draw_ellipse(true),
        };
        kernel
    }

    fn draw_ellipse(&mut self, anti_alias: bool) {
        let (ox, oy) = (self.width / 2, self.height / 2);
        let (px, py) = (1 - self.width % 2, 1 - self.height % 2);
        let (a, b) = if anti_alias {
            (
                (self.width - 2) as f32 / 2.0,
                (self.height - 2) as f32 / 2.0,
            )
        } else {
            (self.width as f32 / 2.0, self.height as f32 / 2.0)
        };
        let (a2, b2) = (a.powi(2), b.powi(2));
        {
            let quarter = (a2 / (a2 + b2).sqrt()).round();
            let mut x = 0.0;
            while x <= quarter {
                let y = b * (1.0 - x.powi(2) / a2).sqrt();
                let error = y.fract();
                if anti_alias {
                    self.draw_4sym(ox, oy, px, py, x as u32, (y.floor() + 1.0) as u32, error);
                }
                self.fill_4sym(ox, oy, px, py, x as u32, y.floor() as u32);
                x += 1.0;
            }
        }
        {
            let quarter = (b2 / (a2 + b2).sqrt()).round();
            let mut y = 0.0;
            while y <= quarter {
                let x = a * (1.0 - y.powi(2) / b2).sqrt();
                let error = x.fract();
                if anti_alias {
                    self.draw_4sym(ox, oy, px, py, (x.floor() + 1.0) as u32, y as u32, error);
                }
                self.fill_4sym(ox, oy, px, py, x.floor() as u32, y as u32);
                y += 1.0;
            }
        }
    }

    fn draw_4sym(&mut self, ox: u32, oy: u32, px: u32, py: u32, x: u32, y: u32, color: f32) {
        if ox + x >= self.width || oy + y >= self.height {
            return;
        }
        *self.pixel_mut(ox + x, oy + y) = color;
        *self.pixel_mut(ox + x, oy - py - y) = color;
        *self.pixel_mut(ox - px - x, oy + y) = color;
        *self.pixel_mut(ox - px - x, oy - py - y) = color;
    }

    fn fill_4sym(&mut self, ox: u32, oy: u32, px: u32, py: u32, x: u32, y: u32) {
        for k in ox - px - x..=ox + x {
            *self.pixel_mut(k, oy + y) = 1.0;
            *self.pixel_mut(k, oy - py - y) = 1.0;
        }
    }

    /// Returns the dimensions of the image.
    #[inline]
    #[must_use]
    pub const fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}

/// Configuration options regarding behavior of dilation
#[derive(Clone)]
pub struct Dilation<'src, 'ker, P: Pixel> {
    /// A reference to the image to dilate
    src: &'src Image<P>,
    /// A reference to the kernel image
    kernel: &'ker KernelImage,
    /// The position to place the dilated image
    position: (u32, u32),
    /// The position of the anchor within the kernel, e.g., (0.5, 0.5) anchors to the center of the kernel
    anchor: (f64, f64),
}

impl<'src, 'ker, P: Pixel> Dilation<'src, 'ker, P> {
    /// Creates a new [`Dilation`] with default settings.
    #[must_use]
    pub fn new(src: &'src Image<P>, kernel: &'ker KernelImage) -> Self {
        Self {
            src,
            kernel,
            position: (0, 0),
            anchor: (0.5, 0.5),
        }
    }

    /// Sets the position to place the dilated image
    #[must_use]
    pub const fn with_position(mut self, x: u32, y: u32) -> Self {
        self.position = (x, y);
        self
    }

    /// Sets the anchor for the dilation, e.g. (0.5, 0.5) anchors to the center of the kernel.
    #[must_use]
    pub const fn with_anchor(mut self, x: f64, y: f64) -> Self {
        self.anchor = (x, y);
        self
    }
}

impl<'src, 'ker, P: Pixel> Draw<P> for Dilation<'src, 'ker, P> {
    fn draw<I: DerefMut<Target = Image<P>>>(&self, mut image: I) {
        let src = self.src;
        let kernel = self.kernel;

        let (w, h) = src.dimensions();
        let (kw, kh) = kernel.dimensions();
        let (ax, ay) = self.anchor;
        let (ax, ay) = ((kw as f64 * ax) as u32, (kh as f64 * ay) as u32);
        let (x1, y1) = self.position;
        let (x2, y2) = (x1 + w, y1 + h);

        let d = |x: u32, y: u32| -> P {
            let mut m: P = P::default();
            for ky in 0..kh {
                for kx in 0..kw {
                    if let Some(k) = kernel.get_pixel(kx, ky).copied() {
                        if let (Some(sx), Some(sy)) =
                            ((x + kx).checked_sub(ax), (y + ky).checked_sub(ay))
                        {
                            m = m.max(
                                src.get_pixel(sx, sy)
                                    .copied()
                                    .map(|p| p * k)
                                    .unwrap_or(P::default()),
                            );
                        }
                    }
                }
            }
            m
        };

        for (y, i) in (y1..y2).zip(0..) {
            for (x, j) in (x1..x2).zip(0..) {
                *image.pixel_mut(x, y) = d(j, i);
            }
        }
    }
}

/// Configuration options regarding behavior of erosion
#[derive(Clone)]
pub struct Erosion<'src, 'ker, P: Pixel> {
    /// A reference to the image to erode
    src: &'src Image<P>,
    /// A reference to the kernel image
    kernel: &'ker KernelImage,
    /// The position to place the eroded image
    position: (u32, u32),
    /// The position of the anchor within the kernel, e.g., (0.5, 0.5) anchors to the center of the kernel
    anchor: (f64, f64),
}

impl<'src, 'ker, P: Pixel> Erosion<'src, 'ker, P> {
    /// Creates a new [`Erosion`] with default settings.
    #[must_use]
    pub fn new(src: &'src Image<P>, kernel: &'ker KernelImage) -> Self {
        Self {
            src,
            kernel,
            position: (0, 0),
            anchor: (0.5, 0.5),
        }
    }

    /// Sets the position to place the dilated image
    #[must_use]
    pub const fn with_position(mut self, x: u32, y: u32) -> Self {
        self.position = (x, y);
        self
    }

    /// Sets the anchor for the dilation, e.g. (0.5, 0.5) anchors to the center of the kernel.
    #[must_use]
    pub const fn with_anchor(mut self, x: f64, y: f64) -> Self {
        self.anchor = (x, y);
        self
    }
}

impl<'src, 'ker, P: Pixel> Draw<P> for Erosion<'src, 'ker, P> {
    fn draw<I: DerefMut<Target = Image<P>>>(&self, mut image: I) {
        let src = self.src;
        let kernel = self.kernel;

        let (w, h) = src.dimensions();
        let (kw, kh) = kernel.dimensions();
        let (ax, ay) = self.anchor;
        let (ax, ay) = ((kw as f64 * ax) as u32, (kh as f64 * ay) as u32);
        let (x1, y1) = self.position;
        let (x2, y2) = (x1 + w, y1 + h);

        let d = |x: u32, y: u32| -> P {
            let mut m: P = !P::default();
            for ky in 0..kh {
                for kx in 0..kw {
                    if let Some(k) = kernel.get_pixel(kx, ky).copied() {
                        if !(k > 0.0) {
                            continue;
                        }
                        if let (Some(sx), Some(sy)) =
                            ((x + kx).checked_sub(ax), (y + ky).checked_sub(ay))
                        {
                            m = m.min(
                                src.get_pixel(sx, sy)
                                    .copied()
                                    .map(|p| p * k)
                                    .unwrap_or(P::default()),
                            );
                        }
                    }
                }
            }
            m
        };

        for (y, i) in (y1..y2).zip(0..) {
            for (x, j) in (x1..x2).zip(0..) {
                *image.pixel_mut(x, y) = d(j, i);
            }
        }
    }
}

/// Creates stroke around an [`Image<Rgba>`].
#[derive(Clone)]
pub struct Stroke {
    /// The alpha channel of a reference image.
    pub alpha: Image<L>,
    /// Stroke size in pixels
    pub size: u32,
    /// Stroke fill color
    pub color: Rgba,
    /// The alpha threshold above which the pixel is considered filled
    pub threshold: u8,
}

impl Stroke {
    /// Creates a new image stroke, with the position default to `(0, 0)` and threshold default to `0`
    #[must_use]
    pub fn new(image: &Image<Rgba>, size: u32, color: Rgba) -> Self {
        Self {
            alpha: image.band(3),
            size: size.max(1),
            color,
            threshold: 0,
        }
    }

    /// Sets the alpha threshold value above which pixels are considered filled.
    #[must_use]
    pub const fn with_threshold(mut self, threshold: u8) -> Self {
        self.threshold = threshold;
        self
    }
}

impl Draw<Rgba> for Stroke {
    fn draw<I: DerefMut<Target = Image<Rgba>>>(&self, mut image: I) {
        let (w, h) = self.alpha.dimensions();
        let src_alpha = self
            .alpha
            .clone()
            .map_pixels(|p| L(if p.0 > self.threshold { 255 } else { 0 }));

        let k_size = self.size * 2 - 1;
        let kernel = KernelImage::from_shape(KernelShape::EllipseAa, k_size, k_size);

        let mut stroke_alpha = Image::new(w, h, L(0));

        stroke_alpha.draw(&Dilation::new(&src_alpha, &kernel));

        let Rgba { r, g, b, a } = self.color;
        let stroke_rgba = stroke_alpha
            .map_pixels(|L(k)| Rgba::new(r, g, b, (a as f32 * k as f32 / u8::MAX as f32) as u8));

        for y in 0..h {
            for x in 0..w {
                if let Some(pixel) = stroke_rgba.get_pixel(x, y) {
                    image.underlay_pixel(x as u32, y as u32, *pixel);
                }
            }
        }
    }
}
