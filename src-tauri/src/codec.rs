//! Port of the macOS `Codec.swift`. The wire format is kept byte-for-byte
//! identical so a given password produces the same 7-symbol string on both
//! platforms (see the golden-vector tests at the bottom).
//!
//! Three stages: ① SHA-256 counter-mode keystream XOR, ② bidirectional
//! additive diffusion (mod 256), ③ base-7 packing into `F U C K Y O u`.

use sha2::{Digest, Sha256};

const ALPHABET: [char; 7] = ['F', 'U', 'C', 'K', 'Y', 'O', 'u'];
const IV_FORWARD: u8 = 0x9E;
const IV_BACKWARD: u8 = 0x7F;

#[derive(Debug, PartialEq, Eq)]
pub enum CodecError {
    InvalidLength,
    InvalidCharacter(char),
    InvalidByte,
    NotUtf8,
}

impl std::fmt::Display for CodecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CodecError::InvalidLength => write!(f, "輸入長度必須是 3 的倍數，這不是合法的編碼字串。"),
            CodecError::InvalidCharacter(c) => {
                write!(f, "包含非法字元「{c}」，編碼字串只能由 F U C K Y O u 組成。")
            }
            CodecError::InvalidByte => write!(f, "包含無效的編碼組合，這不是由 FQEncoder 產生的字串。"),
            CodecError::NotUtf8 => {
                write!(f, "解碼失敗，可能是密碼不正確，或這不是 FQEncoder 編出來的字串。")
            }
        }
    }
}

impl std::error::Error for CodecError {}

fn value_of(c: char) -> Option<u16> {
    ALPHABET.iter().position(|&a| a == c).map(|i| i as u16)
}

// MARK: - Public API

pub fn encode(input: &str, key: &str) -> String {
    let mut bytes = input.as_bytes().to_vec();
    apply_keystream(&mut bytes, key);
    diffuse(&mut bytes);
    pack_base7(&bytes)
}

pub fn decode(input: &str, key: &str) -> Result<String, CodecError> {
    let mut bytes = unpack_base7(input)?;
    undiffuse(&mut bytes);
    apply_keystream(&mut bytes, key);
    String::from_utf8(bytes).map_err(|_| CodecError::NotUtf8)
}

pub fn looks_encoded(input: &str, key: &str) -> bool {
    if input.is_empty() || input.chars().count() % 3 != 0 {
        return false;
    }
    if input.chars().any(|c| value_of(c).is_none()) {
        return false;
    }
    decode(input, key).is_ok()
}

// MARK: - Keystream (SHA-256 counter mode)

fn apply_keystream(bytes: &mut [u8], key: &str) {
    if bytes.is_empty() {
        return;
    }
    let seed = Sha256::digest(format!("FQEncoder.v1:{key}").as_bytes());
    let mut produced = 0usize;
    let mut counter: u64 = 0;
    while produced < bytes.len() {
        let mut hasher = Sha256::new();
        hasher.update(seed);
        hasher.update(counter.to_le_bytes());
        for b in hasher.finalize() {
            if produced >= bytes.len() {
                break;
            }
            bytes[produced] ^= b;
            produced += 1;
        }
        counter = counter.wrapping_add(1);
    }
}

// MARK: - Bidirectional diffusion (O(n), reversible mod 256)

fn diffuse(b: &mut [u8]) {
    if b.is_empty() {
        return;
    }
    let mut acc = IV_FORWARD;
    for x in b.iter_mut() {
        *x = x.wrapping_add(acc);
        acc = *x;
    }
    acc = IV_BACKWARD;
    for x in b.iter_mut().rev() {
        *x = x.wrapping_add(acc);
        acc = *x;
    }
}

fn undiffuse(b: &mut [u8]) {
    if b.is_empty() {
        return;
    }
    // Invert backward pass.
    let mut next = IV_BACKWARD;
    for x in b.iter_mut().rev() {
        let cur = *x;
        *x = cur.wrapping_sub(next);
        next = cur;
    }
    // Invert forward pass.
    let mut prev = IV_FORWARD;
    for x in b.iter_mut() {
        let cur = *x;
        *x = cur.wrapping_sub(prev);
        prev = cur;
    }
}

// MARK: - Base-7 packing

fn pack_base7(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 3);
    for &byte in bytes {
        let v = byte as usize;
        out.push(ALPHABET[(v / 49) % 7]);
        out.push(ALPHABET[(v / 7) % 7]);
        out.push(ALPHABET[v % 7]);
    }
    out
}

fn unpack_base7(input: &str) -> Result<Vec<u8>, CodecError> {
    let chars: Vec<char> = input.chars().collect();
    if chars.len() % 3 != 0 {
        return Err(CodecError::InvalidLength);
    }
    let mut bytes = Vec::with_capacity(chars.len() / 3);
    for chunk in chars.chunks(3) {
        let d1 = value_of(chunk[0]).ok_or(CodecError::InvalidCharacter(chunk[0]))?;
        let d2 = value_of(chunk[1]).ok_or(CodecError::InvalidCharacter(chunk[1]))?;
        let d3 = value_of(chunk[2]).ok_or(CodecError::InvalidCharacter(chunk[2]))?;
        let value = d1 * 49 + d2 * 7 + d3;
        if value > 255 {
            return Err(CodecError::InvalidByte);
        }
        bytes.push(value as u8);
    }
    Ok(bytes)
}

// MARK: - Tests

#[cfg(test)]
mod tests {
    use super::*;

    /// Authoritative outputs produced by compiling the real macOS Codec.swift.
    /// (key, plaintext, encoded)
    const GOLDEN: &[(&str, &str, &str)] = &[
        ("pw", "Hello", "CuOFFFFKUCOFFFY"),
        ("secret", "你好 🌍", "YCCCYFUYYCOOCuOUuYCCKKCYOFuCUuYFK"),
        ("", "A", "YOu"),
        ("hunter2", "FQEncoder", "CYKUKOCCCFuUCUYOFuKCYCCFYYO"),
        ("anything", "", ""),
        (
            "key123",
            "The quick brown fox",
            "OUUYCYFKCFuOFOYUuUKCCYFUUuFKUFCFKYUYKCCKuKUYYYOKKUuCCOUYF",
        ),
    ];

    #[test]
    fn matches_swift_golden_vectors() {
        for (key, plain, encoded) in GOLDEN {
            assert_eq!(&encode(plain, key), encoded, "encode mismatch for {plain:?}");
            // Rust must also decode the Swift-produced string back to plaintext.
            assert_eq!(&decode(encoded, key).unwrap(), plain, "decode mismatch for {encoded:?}");
        }
    }

    #[test]
    fn round_trips() {
        for s in ["", "A", "Hello, World!", "日本語テスト🌸", "FUCKYOu"] {
            let e = encode(s, "round-trip-key");
            assert_eq!(decode(&e, "round-trip-key").unwrap(), s);
        }
    }

    #[test]
    fn wrong_password_rejected() {
        let e = encode("天才", "right");
        assert!(decode(&e, "wrong").is_err());
    }

    #[test]
    fn avalanche_no_shared_prefix() {
        let a = encode("A", "k");
        let ab = encode("AB", "k");
        assert!(!ab.starts_with(&a), "similar inputs must not share an output prefix");
    }

    #[test]
    fn validation_errors() {
        assert_eq!(decode("FF", "k"), Err(CodecError::InvalidLength));
        assert_eq!(decode("FFx", "k"), Err(CodecError::InvalidCharacter('x')));
        assert_eq!(decode("uuu", "k"), Err(CodecError::InvalidByte)); // 6*49+6*7+6 = 342 > 255
        assert!(!looks_encoded("hello world", "k"));
    }
}
