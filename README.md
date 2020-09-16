adc-rs
======
![Build and Test](https://github.com/citruz/adc-rs/workflows/Build%20and%20Test/badge.svg?branch=main)[![crates.io](https://img.shields.io/crates/v/adc)](https://crates.io/crates/adc)

A native rust implementation of the Apple Data Compression scheme used for example in DMG images.
Supports decompression only.

[Documentation](https://docs.rs/adc)

```toml
# Cargo.toml
[dependencies]
adc = "0.2"
```

## Example

```rust
use adc::AdcDecoder;
use std::io::Read;

let input: &[u8] = &[0x83, 0xfe, 0xed, 0xfa, 0xce, 0x00, 0x00, 0x40, 0x00, 0x06];
let mut d = AdcDecoder::new(input);
let mut data = vec![0; 11];
let bytes_out = match d.read_exact(&mut data) {
    Ok(val) => val,
    Err(err) => panic!("error: {:?}", err),
};
println!("{:?} bytes decompressed", bytes_out);
```

Changelog
---------

0.2.1
- Fixed two decoding bugs

0.2.0
- Switched to an API based on the `Read` trait (breaking change)

0.1.0
- Initial release