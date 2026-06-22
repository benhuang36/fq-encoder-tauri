//! Zero-width steganography: hide a secret string inside innocuous-looking
//! cover text. The secret's UTF-8 bytes are written as invisible zero-width
//! characters (ZWSP = bit 0, ZWNJ = bit 1) wrapped in ZWJ sentinels, then
//! tucked in after the cover's first visible character. The result renders
//! identically to the cover text but carries the hidden payload.
//!
//! Framework-agnostic (no Tauri imports) and unit-tested, mirroring `codec.rs`.

const ZERO: char = '\u{200B}'; // ZERO WIDTH SPACE        -> bit 0
const ONE: char = '\u{200C}'; // ZERO WIDTH NON-JOINER    -> bit 1
const MARK: char = '\u{200D}'; // ZERO WIDTH JOINER        -> start/end sentinel

#[derive(Debug, PartialEq, Eq)]
pub enum StegoError {
    NoPayload,
    Corrupt,
    NotUtf8,
}

impl StegoError {
    /// Stable code for the frontend to localise.
    pub fn code(&self) -> &'static str {
        match self {
            StegoError::NoPayload => "stego_no_payload",
            StegoError::Corrupt => "stego_corrupt",
            StegoError::NotUtf8 => "stego_not_utf8",
        }
    }
}

/// Encode `secret` as an invisible zero-width run wrapped in sentinels.
fn payload(secret: &str) -> String {
    let mut out = String::new();
    out.push(MARK);
    for byte in secret.as_bytes() {
        for i in (0..8).rev() {
            out.push(if (byte >> i) & 1 == 1 { ONE } else { ZERO });
        }
    }
    out.push(MARK);
    out
}

/// Hide `secret` inside `cover`. The output looks exactly like `cover`.
pub fn hide(secret: &str, cover: &str) -> String {
    let load = payload(secret);
    let mut chars = cover.chars();
    match chars.next() {
        Some(first) => {
            let mut out = String::new();
            out.push(first);
            out.push_str(&load);
            out.extend(chars);
            out
        }
        None => load, // empty cover -> a fully invisible message
    }
}

/// Extract a secret previously hidden with `hide`.
pub fn reveal(stego: &str) -> Result<String, StegoError> {
    // Collect the bit run between the first two ZWJ sentinels.
    let mut bits: Vec<u8> = Vec::new();
    let mut started = false;
    let mut closed = false;
    for c in stego.chars() {
        match c {
            MARK if !started => started = true,
            MARK if started => {
                closed = true;
                break;
            }
            ZERO if started => bits.push(0),
            ONE if started => bits.push(1),
            _ => {}
        }
    }
    if !started || !closed {
        return Err(StegoError::NoPayload);
    }
    if bits.is_empty() || bits.len() % 8 != 0 {
        return Err(StegoError::Corrupt);
    }
    let bytes: Vec<u8> = bits
        .chunks(8)
        .map(|byte| byte.iter().fold(0u8, |acc, &b| (acc << 1) | b))
        .collect();
    String::from_utf8(bytes).map_err(|_| StegoError::NotUtf8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        for (secret, cover) in [
            ("FUCKYOu", "Have a nice day!"),
            ("你好世界", "Looks totally normal."),
            ("🌸", "hi"),
            ("x", "a"),
        ] {
            let s = hide(secret, cover);
            assert_eq!(reveal(&s).unwrap(), secret);
        }
    }

    #[test]
    fn output_looks_like_cover() {
        let cover = "Have a nice day!";
        let s = hide("secret", cover);
        // Stripping the zero-width characters yields the original cover.
        let visible: String = s.chars().filter(|&c| c != ZERO && c != ONE && c != MARK).collect();
        assert_eq!(visible, cover);
    }

    #[test]
    fn empty_cover_is_all_invisible() {
        let s = hide("hi", "");
        assert!(s.chars().all(|c| c == ZERO || c == ONE || c == MARK));
        assert_eq!(reveal(&s).unwrap(), "hi");
    }

    #[test]
    fn no_payload_errors() {
        assert_eq!(reveal("just plain text"), Err(StegoError::NoPayload));
    }
}
