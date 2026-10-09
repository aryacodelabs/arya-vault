//! ISO/IEC 7816-4 padding to a multiple of 1 KiB (docs/04 §6, SEC-Y02).
//!
//! `pad` appends `0x80` followed by zero bytes up to the next multiple of 1024, so there
//! is always at least one padding byte (a 1024-byte plaintext becomes 2048 bytes).
//! `unpad` is strict: it requires the exact, minimal form that `pad` produces, and is
//! only ever run on **authenticated** plaintext.

use zeroize::Zeroizing;

use super::FormatError;

/// Padding bucket size in bytes.
pub const BUCKET: usize = 1024;

/// Length of the padded form of an `n`-byte plaintext (always `> n`).
pub const fn padded_len(n: usize) -> usize {
    (n / BUCKET + 1) * BUCKET
}

/// Pads `data` to the next multiple of [`BUCKET`]. The result is wiped on drop.
pub fn pad(data: &[u8]) -> Zeroizing<Vec<u8>> {
    let mut out = Zeroizing::new(Vec::with_capacity(padded_len(data.len())));
    out.extend_from_slice(data);
    out.push(0x80);
    out.resize(padded_len(data.len()), 0);
    out
}

/// Removes padding, returning the plaintext slice.
///
/// Rejects: empty or non-multiple-of-1024 input, input without a `0x80` marker after the
/// trailing zeros, and padding that is longer than the minimal form.
pub fn unpad(padded: &[u8]) -> Result<&[u8], FormatError> {
    if padded.is_empty() || padded.len() % BUCKET != 0 {
        return Err(FormatError::Padding);
    }
    let end = padded
        .iter()
        .rposition(|b| *b != 0)
        .ok_or(FormatError::Padding)?;
    if padded[end] != 0x80 || padded_len(end) != padded.len() {
        return Err(FormatError::Padding);
    }
    Ok(&padded[..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    // SEC-Y02: boundaries 0, 1, 1023, 1024, 1025 and beyond.
    #[test]
    fn padded_lengths_at_boundaries() {
        for (n, want) in [
            (0, 1024),
            (1, 1024),
            (1022, 1024),
            (1023, 1024),
            (1024, 2048),
            (1025, 2048),
            (2047, 2048),
            (2048, 3072),
        ] {
            assert_eq!(padded_len(n), want, "n={n}");
            let p = pad(&vec![7u8; n]);
            assert_eq!(p.len(), want, "n={n}");
            assert_eq!(unpad(&p).unwrap(), vec![7u8; n].as_slice(), "n={n}");
        }
    }

    #[test]
    fn padding_bytes_are_80_then_zeros() {
        let p = pad(b"abc");
        assert_eq!(&p[..3], b"abc");
        assert_eq!(p[3], 0x80);
        assert!(p[4..].iter().all(|b| *b == 0));
    }

    #[test]
    fn plaintext_ending_in_zero_or_80_round_trips() {
        for tail in [0x00u8, 0x80] {
            let data = vec![tail; 10];
            assert_eq!(unpad(&pad(&data)).unwrap(), data.as_slice());
        }
    }

    #[test]
    fn malformed_padding_is_rejected() {
        // Empty, wrong multiple.
        assert!(unpad(&[]).is_err());
        assert!(unpad(&[0x80]).is_err());
        assert!(unpad(&vec![0u8; 1023]).is_err());
        // All zeros (no marker).
        assert!(unpad(&vec![0u8; 1024]).is_err());
        // Wrong marker byte.
        let mut p = pad(b"abc").to_vec();
        p[3] = 0x81;
        assert!(unpad(&p).is_err());
        // Trailing non-zero after marker.
        let mut p = pad(b"abc").to_vec();
        p[1023] = 1;
        assert!(unpad(&p).is_err());
        // Non-minimal: a whole extra bucket of padding.
        let mut p = pad(b"abc").to_vec();
        p.extend(vec![0u8; 1024]);
        assert!(unpad(&p).is_err());
        // Marker present but data ends exactly at bucket boundary with no room: 1024
        // bytes of data followed by 1024 of padding is fine, but 1024 bytes ending in 0x80
        // is the marker of a 1023-byte plaintext only if the rest is zero.
        let mut p = vec![1u8; 1023];
        p.push(0x80);
        assert_eq!(unpad(&p).unwrap().len(), 1023);
    }

    #[test]
    fn unpad_never_panics_on_arbitrary_input() {
        for len in [0usize, 1, 2, 1023, 1024, 1025, 2048] {
            for fill in [0u8, 1, 0x80, 0xff] {
                let _ = unpad(&vec![fill; len]);
            }
        }
    }
}
