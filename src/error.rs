use std::ffi::OsString;
use std::fmt;

pub type Result<T> = std::result::Result<T, Error>;

/// Represents an error that occurs within the crate.
#[derive(Debug)]
pub enum Error {
    /// An invalid hex code was provided when trying to parse a hex value.
    InvalidHexCode(String),

    /// An invalid extension was provided when trying to resolve an image's encoding format
    /// from a file extension.
    ///
    /// # Note
    /// This is **not** an error that occurs when the file extension is not recognized, or
    /// is an unknown image extension. This occurs if the OsStr fails conversion to a native
    /// &str. In the case of this, [`ImageFormat::Unknown`] is used instead.
    InvalidExtension(OsString),

    /// Invalid data was encountered when an image, usually because it is corrupted.
    ///
    /// Errors can differ across encodings, so the inner ``&'static str`` here is nothing more than
    /// an error message.
    DecodingError(&'static str),

    /// An image received data incompatible with the image's dimensions.
    IncompatibleImageData {
        width: u32,
        height: u32,
        received: usize,
    },

    /// Received an unsupported color type when trying to create a pixel from raw data.
    ///
    /// This occurs when the color type is not supported by the pixel type. This is almost
    /// always fixed by switching the pixel type to [`Dynamic`] then using [`Image::convert`]
    /// to convert the image into your desired type.
    UnsupportedColorType,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::InvalidHexCode(hex_code) => write!(f, "Invalid hex code: {}", hex_code),
            Error::InvalidExtension(ext) => {
                write!(f, "Invalid extension: {}", ext.to_string_lossy())
            }
            Error::DecodingError(message) => write!(f, "Decoding error: {}", message),
            Error::UnsupportedColorType => write!(
                f,
                "Unsupported color type. Try using the `Dynamic` pixel type instead."
            ),
            Error::IncompatibleImageData {
                width,
                height,
                received,
            } => write!(
                f,
                "An image with dimensions {}x{} should have {} pixels, received {} instead",
                width,
                height,
                width * height,
                received,
            ),
        }
    }
}