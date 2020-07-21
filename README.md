adc-rs
======
A native rust implementation of the Apple Data Compression scheme used for example in DMG images.
Supports decompression only.

[Documentation](https://docs.rs/adc)

```toml
# Cargo.toml
[dependencies]
adc = "0.1.0"
```

## Example

```rust
use adc::AdcDecoder;

let input: &[u8] = &[0x83, 0xfe, 0xed, 0xfa, 0xce, 0x00, 0x00, 0x40, 0x00, 0x06];
let mut d = AdcDecoder::new(input);
let mut data = vec![0; 11];
let bytes_out = match d.decompress_into(&mut data[..]) {
    Ok(val) => val,
    Err(err) => panic!("error: {:?}", err),
};
println!("{:?} bytes decompressed", bytes_out);
```
