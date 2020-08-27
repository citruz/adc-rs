//! Implementation of the Apple Data Compression scheme in Rust
//!
//! ADC is a rather basic run length compression scheme. This library implements decompression only.
//!
//! # Example
//!
//! ```
//! use adc::AdcDecoder;
//!
//! let input: &[u8] = &[0x83, 0xfe, 0xed, 0xfa, 0xce, 0x00, 0x00, 0x40, 0x00, 0x06];
//! let mut d = AdcDecoder::new(input);
//! let mut data = vec![0; 11];
//! let bytes_out = match d.decompress_into(&mut data[..]) {
//!     Ok(val) => val,
//!     Err(err) => panic!("error: {:?}", err),
//! };
//! println!("{:?} bytes decompressed", bytes_out);
//! ````

use std::io::prelude::*;

use bincode::Options;

#[derive(PartialEq, Debug)]
enum AdcChunkType {
    Plain,
    TwoByte,
    ThreeByte,
}

#[derive(PartialEq, Debug)]
struct AdcChunk {
    r#type: AdcChunkType,
    size: usize,
    offset: usize,
}

/// Main type for decompressing ADC data.
pub struct AdcDecoder<R> {
    input: R,
}

/// This type represents all possible errors that can occur when decompressing data.
#[derive(Debug, PartialEq)]
pub enum AdcError {
    /// The was an IO error while reading or writing.
    Io(String),
    /// The buffer was not large enough for the decompressed data.
    BufferTooSmall,
    /// The input is invalid.
    InvalidInput,
}

impl std::fmt::Display for AdcError {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            AdcError::Io(ref err) => write!(fmt, "{}", err),
            AdcError::BufferTooSmall => write!(fmt, "output buffer too small"),
            AdcError::InvalidInput => write!(fmt, "invalid input data"),
        }
    }
}

impl<R: Read> AdcDecoder<R> {
    /// Create a new decoder instance from a readable input
    pub fn new(input: R) -> AdcDecoder<R> {
        AdcDecoder { input }
    }

    fn get_next_chunk(&mut self) -> Result<Option<AdcChunk>, AdcError> {
        let byte: u8 = match bincode::deserialize_from(&mut self.input) {
            Err(_) => return Ok(None), // reached eof
            Ok(val) => val,
        };
        let chunk_type = if (byte & 0x80) != 0 {
            AdcChunkType::Plain
        } else if (byte & 0x40) != 0 {
            AdcChunkType::ThreeByte
        } else {
            AdcChunkType::TwoByte
        };

        let chunk = match chunk_type {
            AdcChunkType::Plain => AdcChunk {
                r#type: chunk_type,
                size: ((byte & 0x7f) + 1) as usize,
                offset: 0,
            },
            AdcChunkType::TwoByte => {
                let byte2: u8 = match bincode::deserialize_from(&mut self.input) {
                    Err(err) => return Err(AdcError::Io(format!("{}", err))),
                    Ok(val) => val,
                };
                AdcChunk {
                    r#type: chunk_type,
                    size: (((byte & 0x3f) >> 2) + 3) as usize,
                    offset: (((byte as usize) & 0x3) << 8) + byte2 as usize,
                }
            }
            AdcChunkType::ThreeByte => {
                let offset: u16 = match bincode::DefaultOptions::new()
                    .with_big_endian()
                    .with_fixint_encoding()
                    .deserialize_from(&mut self.input)
                {
                    Err(err) => return Err(AdcError::Io(format!("{}", err))),
                    Ok(val) => val,
                };
                AdcChunk {
                    r#type: chunk_type,
                    size: ((byte & 0x3f) + 4) as usize,
                    offset: offset as usize,
                }
            }
        };

        Ok(Some(chunk))
    }

    /// Decompress input into byte array
    pub fn decompress_into(&mut self, output: &mut [u8]) -> Result<usize, AdcError> {
        // ADC is basic run length compression. AdcChunkType::Plain chunk contains data, the other two
        // specify the run length and an optional offset.
        let mut cur_pos = 0;
        loop {
            // get next chunk or return if None
            let chunk = match self.get_next_chunk()? {
                None => return Ok(cur_pos),
                Some(val) => val,
            };

            if cur_pos + chunk.size > output.len() {
                return Err(AdcError::BufferTooSmall);
            }

            if chunk.r#type == AdcChunkType::Plain {
                // copy from input to output
                if let Err(err) = self
                    .input
                    .read_exact(&mut output[cur_pos..cur_pos + chunk.size])
                {
                    return Err(AdcError::Io(format!("{}", err)));
                }
                cur_pos += chunk.size;
            } else {
                if cur_pos == 0 || chunk.offset > cur_pos - 1 {
                    return Err(AdcError::InvalidInput);
                }
                // copy repeated bytes
                for _ in 0..chunk.size {
                    output[cur_pos] = output[cur_pos - chunk.offset - 1];
                    cur_pos += 1;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_types() {
        let input: &[u8] = &[0x83, 0xfe, 0xed, 0xfa, 0xce, 0x00, 0x00, 0x40, 0x00, 0x06];
        let output: &[u8] = &[
            0xfe, 0xed, 0xfa, 0xce, 0xce, 0xce, 0xce, 0xfe, 0xed, 0xfa, 0xce,
        ];
        let mut d = AdcDecoder::new(input);
        let mut data = vec![0; output.len()];
        let bytes_out = d.decompress_into(&mut data[..]).unwrap();
        assert_eq!(bytes_out, output.len());
        assert_eq!(output[..], data[..]);
    }

    #[test]
    fn invalid_input() {
        // offset is too big
        let input: &[u8] = &[0x83, 0xfe, 0xed, 0xfa, 0xce, 0x00, 0xff];
        let mut d = AdcDecoder::new(input);
        let mut data = vec![0; 10];
        let bytes_out = d.decompress_into(&mut data[..]);
        assert_eq!(bytes_out, Err(AdcError::InvalidInput));
    }

    #[test]
    fn invalid_input2() {
        // run-length chunk at position 0
        let input: &[u8] = &[0x00, 0x00];
        let mut d = AdcDecoder::new(input);
        let mut data = vec![0; 10];
        let bytes_out = d.decompress_into(&mut data[..]);
        assert_eq!(bytes_out, Err(AdcError::InvalidInput));
    }

    #[test]
    fn invalid_input3() {
        // missing 2nd byte
        let input: &[u8] = &[0x83, 0xfe, 0xed, 0xfa, 0xce, 0x00];
        let mut d = AdcDecoder::new(input);
        let mut data = vec![0; 10];
        let bytes_out = d.decompress_into(&mut data[..]);
        assert_eq!(
            bytes_out,
            Err(AdcError::Io(
                "io error: failed to fill whole buffer".to_string()
            ))
        );
    }

    #[test]
    fn empty() {
        let input: &[u8] = &[];
        let output: &[u8] = &[];
        let mut d = AdcDecoder::new(input);
        let mut data = vec![0; output.len()];
        let bytes_out = d.decompress_into(&mut data[..]).unwrap();
        assert_eq!(bytes_out, output.len());
        assert_eq!(output[..], data[..]);
    }
}
