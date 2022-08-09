# ril
**R**ust **I**maging **L**ibrary: A performant and high-level Rust imaging crate.

## What's this?
This is a Rust crate designed to provide an easy-to-use, high-level interface
around image processing in Rust. Image and animation processing has never been
this easy before, and it's hard to find a good crate for it.

RIL was designed not only for static single-frame images in mind, but also for
animated images such as GIFs or APNGs that have multiple frames. RIL provides a
streamlined API for this.

Even better, benchmarks prove that RIL, even with its high-level interface, is as
performant and usually even faster than leading imaging crates such as `image-rs`. See
[benchmarks](#benchmarks) for more information.

## Features
- Support for encoding from/decoding to a wide range of image formats
- Variety of image processing and manipulation operations, including drawing
- Robust support for animated images such as GIFs via FrameIterator and ImageSequence
  - See [Animated Image Support](#animated-image-support) for more information.
- A streamlined front-facing interface

## Support
⚠ This crate is a work in progress

By the first stable release, we plan to support the following image encodings:

| Encoding Format | Current Status    |
|-----------------|-------------------|
| PNG/APNG        | Supported         |
| JPEG            | Supported         |
| GIF             | Supported         |
| WebP            | Not yet supported |
| BMP             | Not yet supported |
| TIFF            | Not yet supported |

Additionally, we also plan to support the following pixel formats:

| Pixel Format                           | Current Status          |
|----------------------------------------|-------------------------|
| RGB8                                   | Supported as `Rgb`      |
| RGBA8                                  | Supported as `Rgba`     |
| L8 (grayscale)                         | Supported as `L`        |
| LA8 (grayscale + alpha)                | Not yet supported       |
| 1 (single-bit pixel, equivalent to L1) | Supported as `BitPixel` |

16-bit pixel formats are currently downscaled to 8-bits. We do plan to
have actual support 16-bit pixel formats in the future.

## Requirements
MSRV (Minimum Supported Rust Version) is v1.61.0.

## Installation
Add the following to your `Cargo.toml` dependencies:
```toml
ril = "0"
```

Or, you can run `cargo add ril` if you have Rust 1.62.0 or newer.

## Benchmarks

### Decode GIF + Invert each frame + Encode GIF (600x600, 77 frames)
Performed locally on Apple Macbook Pro 2021 (10-cores) ([Source](https://github.com/jay3332/ril/blob/main/benches/invert_comparison.rs))

| Benchmark                                     | Time (average of 10, lower is better) |
|-----------------------------------------------|---------------------------------------|
| ril (combinator)                              | 902.54 ms                             |
| ril (for-loop)                                | 922.08 ms                             |
| ril (low-level hardcoded GIF en/decoder)      | 902.28 ms                             |
| image-rs (low-level hardcoded GIF en/decoder) | 940.42 ms                             |
| Python, wand (ImageMagick)                    | 1049.09 ms                            |

## Examples

#### Open an image, invert it, and then save it:
```rust
use ril::prelude::*;

fn main() -> ril::Result<()> {
    let image = Image::open("sample.png")?;
    image.invert();
    image.save_inferred("inverted.png")?;
    
    Ok(())
}
```

or, why not use method chaining?
```rust
Image::open("sample.png")?
    .inverted()
    .save_inferred("inverted.png")?;
```

#### Create a new black image, open the sample image, and paste it on top of the black image:
```rust
let image = Image::new(600, 600, Rgb::black());
image.paste(100, 100, Image::open("sample.png")?);
image.save_inferred("sample_on_black.png")?;
```

you can still use method chaining, but this accesses a lower level interface:
```rust
let image = Image::new(600, 600, Rgb::black())
    .with(&Paste::new(Image::open("sample.png")?).with_position(100, 100))
    .save_inferred("sample_on_black.png")?;
```

#### Open an image and mask it to a circle:
```rust
let image = Image::<Rgba>::open("sample.png")?;
let (width, height) = image.dimensions();

let ellipse = 
    Ellipse::from_bounding_box(0, 0, width, height).with_fill(L(255));

let mask = Image::new(width, height, L(0));
mask.draw(&ellipse);

image.mask_alpha(&mask);
image.save_inferred("sample_circle.png")?;
```

### Animated Image Support
RIL supports high-level encoding, decoding, and processing of animated images of any format,
such as GIF or APNGs.

Animated images can be lazily decoded. This means you can process the frames of an animated image
one by one as each frame is decoded. This can lead to huge performance and memory gains when compared to 
decoding all frames at once, processing those frames individually, and then encoding the image back to a file.

For lazy animated image decoding, the `DynamicFrameIterator` is used as a high-level iterator interface
to iterate through all frames of an animated image, lazily. These implement `Iterator<Item = Frame<_>>`.

For times when you need to collect all frames of an image, `ImageSequence` is used as a high-level
interface around a sequence of images. This can hold extra metadata about the animation such as loop count.

#### Open an animated image and invert each frame as they are decoded, then saving them:

```rust
let mut output = ImageSequence::<Rgba>::new();

// ImageSequence::open is lazy
for frame in ImageSequence::<Rgba>::open("sample.gif")? {
    let frame = frame?;
    frame.invert();
    output.push(frame);

    // or...
    output.push_frame(frame?.map_image(|image| image.inverted()));
}

output.save_inferred("inverted.gif")?;
```

#### Open an animated image and save each frame into a separate PNG image as they are decoded:
```rust
ImageSequence::<Rgba>::open("sample.gif")?
    .enumerate()
    .for_each(|(idx, frame)| {
        frame
            .unwrap()
            .save_inferred(format!("frames/{}.png", idx))
            .unwrap();
    });
```

Although a bit misleading a first, `ImageSequence::open` and `ImageSequence::decode_[inferred_]from_bytes`
return lazy `DynamicFrameIterator`s.

Additionally, `Frame`s house `Image`s, but they are not `Image`s themselves. However, `Frame`s are able
to dereference into `Image`s, so calling image methods on frames will seem transparent.
