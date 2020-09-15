//! Implementation of the Apple Data Compression scheme in Rust
//!
//! ADC is a rather basic run length compression scheme. This library implements decompression only.
//!
//! # Example
//!
//! ```
//! use adc::AdcDecoder;
//! use std::io::Read;
//!
//! let input: &[u8] = &[0x83, 0xfe, 0xed, 0xfa, 0xce, 0x00, 0x00, 0x40, 0x00, 0x06];
//! let mut d = AdcDecoder::new(input);
//! let mut data = vec![0; 11];
//! let bytes_out = match d.read_exact(&mut data) {
//!     Ok(val) => val,
//!     Err(err) => panic!("error: {:?}", err),
//! };
//! println!("{:?} bytes decompressed", bytes_out);
//! ````

use byteorder::{ReadBytesExt, BE};
use std::{
    cmp,
    collections::VecDeque,
    io::{self, prelude::*},
    u16,
};

#[derive(PartialEq, Debug)]
enum AdcChunkType {
    Plain,
    TwoByte,
    ThreeByte,
}

#[derive(PartialEq, Debug)]
struct AdcChunk {
    r#type: AdcChunkType,
    size: u8,
    offset: u16,
}

/// Window into the decompressed output.
///
/// Used to get output bytes for the run-length chunks.
/// Implemented as a non-growable ring buffer.
struct Window(VecDeque<u8>);

impl Window {
    // The windows needs to fit `max offset` bytes.
    const SIZE: usize = u16::MAX as usize + 1;

    fn new() -> Self {
        Self(VecDeque::with_capacity(Self::SIZE))
    }

    fn extend(&mut self, bytes: &[u8]) {
        // remove from the back to ensure we have enough room
        let max_size = Self::SIZE - bytes.len();
        self.0.truncate(max_size);

        // push new bytes to the front
        for &byte in bytes {
            self.0.push_front(byte);
        }
    }

    fn get(&self, idx: u16) -> Option<u8> {
        self.0.get(idx as usize).copied()
    }
}

/// Main type for decompressing ADC data.
pub struct AdcDecoder<R> {
    input: R,
    current_chunk: Option<AdcChunk>,
    window: Window,
}

impl<R: Read> AdcDecoder<R> {
    /// Create a new decoder instance from a readable input
    pub fn new(input: R) -> AdcDecoder<R> {
        AdcDecoder {
            input,
            current_chunk: None,
            window: Window::new(),
        }
    }

    /// Update `self.current_chunk` with the next chunk.
    fn next_chunk(&mut self) -> io::Result<()> {
        let byte = match self.input.read_u8() {
            Ok(val) => val,
            Err(_) => {
                self.current_chunk = None;
                return Ok(());
            }
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
                size: (byte & 0x7f) + 1,
                offset: 0,
            },
            AdcChunkType::TwoByte => {
                let byte2 = self.input.read_u8()?;
                AdcChunk {
                    r#type: chunk_type,
                    size: ((byte & 0x3f) >> 2) + 3,
                    offset: ((u16::from(byte) & 0x3) << 8) + u16::from(byte2),
                }
            }
            AdcChunkType::ThreeByte => {
                let offset = self.input.read_u16::<BE>()?;
                AdcChunk {
                    r#type: chunk_type,
                    size: (byte & 0x3f) + 4,
                    offset,
                }
            }
        };

        self.current_chunk = Some(chunk);
        Ok(())
    }

    fn read_from_chunk(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let chunk = match self.current_chunk {
            Some(ref mut c) => c,
            None => return Ok(0),
        };

        let read_len = cmp::min(chunk.size as usize, buf.len());
        let buf = &mut buf[..read_len];

        if chunk.r#type == AdcChunkType::Plain {
            self.input.read_exact(buf)?;
            self.window.extend(buf);
        } else {
            // read run of bytes from the output window
            for elem in buf.iter_mut() {
                let byte = match self.window.get(chunk.offset) {
                    Some(b) => b,
                    None => {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "invalid chunk offset",
                        ))
                    }
                };

                *elem = byte;
                self.window.extend(&[byte]);
            }
        }

        chunk.size -= read_len as u8;
        if chunk.size == 0 {
            self.current_chunk = None;
        }

        Ok(read_len)
    }
}

impl<R: Read> Read for AdcDecoder<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.current_chunk.is_none() {
            self.next_chunk()?;
        }

        self.read_from_chunk(buf)
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
        d.read_exact(&mut data).unwrap();

        assert_eq!(output[..], data[..]);
    }

    #[test]
    fn invalid_input() {
        // offset is too big
        let input: &[u8] = &[0x83, 0xfe, 0xed, 0xfa, 0xce, 0x00, 0xff];

        let mut d = AdcDecoder::new(input);
        let mut data = vec![0; 10];
        let err = d.read_exact(&mut data).unwrap_err();

        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn invalid_input2() {
        // run-length chunk at position 0
        let input: &[u8] = &[0x00, 0x00];

        let mut d = AdcDecoder::new(input);
        let mut data = vec![0; 10];
        let err = d.read_exact(&mut data).unwrap_err();

        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn invalid_input3() {
        // missing 2nd byte
        let input: &[u8] = &[0x83, 0xfe, 0xed, 0xfa, 0xce, 0x00];

        let mut d = AdcDecoder::new(input);
        let mut data = vec![0; 10];
        let err = d.read_exact(&mut data).unwrap_err();

        assert_eq!(err.kind(), io::ErrorKind::UnexpectedEof);
    }

    #[test]
    fn empty() {
        let input: &[u8] = &[];
        let output: &[u8] = &[];

        let mut d = AdcDecoder::new(input);
        let mut data = vec![0; output.len()];
        d.read_exact(&mut data).unwrap();

        assert_eq!(output[..], data[..]);
    }
}
