use std::fmt;
use std::io::{self, Read};
use std::str::{self, FromStr};

#[derive(Debug)]
pub(crate) enum CharReadError {
    UnexpectedEof,
    Utf8(str::Utf8Error),
    Io(io::Error),
}

impl From<str::Utf8Error> for CharReadError {
    #[cold]
    fn from(e: str::Utf8Error) -> Self {
        Self::Utf8(e)
    }
}

impl From<io::Error> for CharReadError {
    #[cold]
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

impl fmt::Display for CharReadError {
    #[cold]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use self::CharReadError::{Io, UnexpectedEof, Utf8};
        match *self {
            UnexpectedEof => write!(f, "unexpected end of stream"),
            Utf8(ref e) => write!(f, "UTF-8 decoding error: {e}"),
            Io(ref e) => write!(f, "I/O error: {e}"),
        }
    }
}

/// Character encoding used for parsing
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[non_exhaustive]
pub enum Encoding {
    /// Explicitly UTF-8 only
    Utf8,
    /// UTF-8 fallback, but can be any 8-bit encoding
    Default,
    /// ISO-8859-1
    Latin1,
    /// US-ASCII
    Ascii,
    /// Big-Endian
    Utf16Be,
    /// Little-Endian
    Utf16Le,
    /// Unknown endianness yet, will be sniffed
    Utf16,
    /// Not determined yet, may be sniffed to be anything
    Unknown,
}

// Rustc inlines eq_ignore_ascii_case and creates kilobytes of code!
#[inline(never)]
fn icmp(lower: &str, varcase: &str) -> bool {
    lower.bytes().zip(varcase.bytes()).all(|(l, v)| l == v.to_ascii_lowercase())
}

impl FromStr for Encoding {
    type Err = &'static str;

    fn from_str(val: &str) -> Result<Self, Self::Err> {
        if ["utf-8", "utf8"].into_iter().any(move |label| icmp(label, val)) {
            Ok(Self::Utf8)
        } else if ["iso-8859-1", "latin1"].into_iter().any(move |label| icmp(label, val)) {
            Ok(Self::Latin1)
        } else if ["utf-16", "utf16"].into_iter().any(move |label| icmp(label, val)) {
            Ok(Self::Utf16)
        } else if ["ascii", "us-ascii"].into_iter().any(move |label| icmp(label, val)) {
            Ok(Self::Ascii)
        } else {
            Err("unknown encoding name")
        }
    }
}

impl fmt::Display for Encoding {
    #[cold]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Utf8 |
            Self::Default => "UTF-8",
            Self::Latin1 => "ISO-8859-1",
            Self::Ascii => "US-ASCII",
            Self::Utf16Be |
            Self::Utf16Le |
            Self::Utf16 => "UTF-16",
            Self::Unknown => "(unknown)",
        })
    }
}

// CharReader has been removed - use read_char_from function directly

fn surrogate(buf: [u16; 2]) -> Result<Option<char>, CharReadError> {
    char::decode_utf16(buf).next().transpose()
        .map_err(|e| CharReadError::Io(io::Error::new(io::ErrorKind::InvalidData, e)))
}

/// Read a character from a source with the given encoding
#[allow(clippy::unbuffered_bytes)]
pub(crate) fn read_char_from<R: Read>(
    source: &mut R,
    encoding: &mut Encoding,
    buf: &mut [u8; 4],
    pos: &mut usize,
) -> Result<Option<char>, CharReadError> {
    let mut bytes = source.bytes();
    const MAX_CODEPOINT_LEN: usize = 4;

    while *pos < MAX_CODEPOINT_LEN {
        let next = match bytes.next() {
            Some(Ok(b)) => b,
            Some(Err(e)) => return Err(e.into()),
            None if *pos == 0 => return Ok(None),
            None => return Err(CharReadError::UnexpectedEof),
        };

        match *encoding {
            Encoding::Utf8 | Encoding::Default => {
                // fast path for ASCII subset
                if *pos == 0 && next.is_ascii() {
                    return Ok(Some(next.into()));
                }

                buf[*pos] = next;
                *pos += 1;

                match str::from_utf8(&buf[..*pos]) {
                    Ok(s) => return Ok(s.chars().next()), // always Some(..)
                    Err(_) if *pos < MAX_CODEPOINT_LEN => continue,
                    Err(e) => return Err(e.into()),
                }
            },
            Encoding::Latin1 => {
                return Ok(Some(next.into()));
            },
            Encoding::Ascii => {
                return if next.is_ascii() {
                    Ok(Some(next.into()))
                } else {
                    Err(CharReadError::Io(io::Error::new(io::ErrorKind::InvalidData, "char is not ASCII")))
                };
            },
            Encoding::Unknown | Encoding::Utf16 => {
                buf[*pos] = next;
                *pos += 1;
                if let Some(value) = sniff_bom(encoding, &buf[..*pos], pos) {
                    return value;
                }
            },
            Encoding::Utf16Be => {
                buf[*pos] = next;
                *pos += 1;
                if *pos == 2 {
                    if let Some(Ok(c)) = char::decode_utf16([u16::from_be_bytes(buf[..2].try_into().unwrap())]).next() {
                        return Ok(Some(c));
                    }
                } else if *pos == 4 {
                    return surrogate([u16::from_be_bytes(buf[..2].try_into().unwrap()), u16::from_be_bytes(buf[2..4].try_into().unwrap())]);
                }
            },
            Encoding::Utf16Le => {
                buf[*pos] = next;
                *pos += 1;
                if *pos == 2 {
                    if let Some(Ok(c)) = char::decode_utf16([u16::from_le_bytes(buf[..2].try_into().unwrap())]).next() {
                        return Ok(Some(c));
                    }
                } else if *pos == 4 {
                    return surrogate([u16::from_le_bytes(buf[..2].try_into().unwrap()), u16::from_le_bytes(buf[2..4].try_into().unwrap())]);
                }
            },
        }
    }
    Err(CharReadError::Io(io::ErrorKind::InvalidData.into()))
}

#[cold]
fn sniff_bom(encoding: &mut Encoding, buf: &[u8], pos: &mut usize) -> Option<Result<Option<char>, CharReadError>> {
    // sniff BOM
    if buf.len() <= 3 && [0xEF, 0xBB, 0xBF].starts_with(buf) {
        if buf.len() == 3 && *encoding != Encoding::Utf16 {
            *pos = 0;
            *encoding = Encoding::Utf8;
        }
    } else if buf.len() <= 2 && [0xFE, 0xFF].starts_with(buf) {
        if buf.len() == 2 {
            *pos = 0;
            *encoding = Encoding::Utf16Be;
        }
    } else if buf.len() <= 2 && [0xFF, 0xFE].starts_with(buf) {
        if buf.len() == 2 {
            *pos = 0;
            *encoding = Encoding::Utf16Le;
        }
    } else if buf.len() == 1 && *encoding == Encoding::Utf16 {
        // sniff ASCII char in UTF-16
        *encoding = if buf[0] == 0 { Encoding::Utf16Be } else { Encoding::Utf16Le };
    } else {
        // UTF-8 is the default, but XML decl can change it to other 8-bit encoding
        *encoding = Encoding::Default;
        if buf.len() == 1 && buf[0].is_ascii() {
            return Some(Ok(Some(buf[0].into())));
        }
    }
    None
}

#[cfg(feature = "async")]
/// Read a character from an async source with the given encoding
pub(crate) async fn async_read_char_from<R: tokio::io::AsyncRead + Unpin>(
    source: &mut R,
    encoding: &mut Encoding,
    buf: &mut [u8; 4],
    pos: &mut usize,
) -> Result<Option<char>, CharReadError> {
    use tokio::io::AsyncReadExt;
    const MAX_CODEPOINT_LEN: usize = 4;

    while *pos < MAX_CODEPOINT_LEN {
        let next = match source.read_u8().await {
            Ok(b) => b,
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {
                if *pos == 0 {
                    return Ok(None);
                } else {
                    return Err(CharReadError::UnexpectedEof);
                }
            }
            Err(e) => return Err(e.into()),
        };

        match *encoding {
            Encoding::Utf8 | Encoding::Default => {
                // fast path for ASCII subset
                if *pos == 0 && next.is_ascii() {
                    return Ok(Some(next.into()));
                }

                buf[*pos] = next;
                *pos += 1;

                match str::from_utf8(&buf[..*pos]) {
                    Ok(s) => return Ok(s.chars().next()), // always Some(..)
                    Err(_) if *pos < MAX_CODEPOINT_LEN => continue,
                    Err(e) => return Err(e.into()),
                }
            },
            Encoding::Latin1 => {
                return Ok(Some(next.into()));
            },
            Encoding::Ascii => {
                return if next.is_ascii() {
                    Ok(Some(next.into()))
                } else {
                    Err(CharReadError::Io(io::Error::new(io::ErrorKind::InvalidData, "char is not ASCII")))
                };
            },
            Encoding::Unknown | Encoding::Utf16 => {
                buf[*pos] = next;
                *pos += 1;
                if let Some(value) = sniff_bom(encoding, &buf[..*pos], pos) {
                    return value;
                }
            },
            Encoding::Utf16Be => {
                buf[*pos] = next;
                *pos += 1;
                if *pos == 2 {
                    if let Some(Ok(c)) = char::decode_utf16([u16::from_be_bytes(buf[..2].try_into().unwrap())]).next() {
                        return Ok(Some(c));
                    }
                } else if *pos == 4 {
                    return surrogate([u16::from_be_bytes(buf[..2].try_into().unwrap()), u16::from_be_bytes(buf[2..4].try_into().unwrap())]);
                }
            },
            Encoding::Utf16Le => {
                buf[*pos] = next;
                *pos += 1;
                if *pos == 2 {
                    if let Some(Ok(c)) = char::decode_utf16([u16::from_le_bytes(buf[..2].try_into().unwrap())]).next() {
                        return Ok(Some(c));
                    }
                } else if *pos == 4 {
                    return surrogate([u16::from_le_bytes(buf[..2].try_into().unwrap()), u16::from_le_bytes(buf[2..4].try_into().unwrap())]);
                }
            },
        }
    }
    Err(CharReadError::Io(io::ErrorKind::InvalidData.into()))
}

#[cfg(test)]
mod tests {
    use super::{CharReadError, Encoding, read_char_from};

    #[test]
    fn test_read_char_from() {
        use std::io;

        let mut bytes: &[u8] = b"correct";    // correct ASCII
        let mut encoding = Encoding::Unknown;
        let mut buf = [0; 4];
        let mut pos = 0;
        assert_eq!(read_char_from(&mut bytes, &mut encoding, &mut buf, &mut pos).unwrap(), Some('c'));

        let mut bytes: &[u8] = b"\xEF\xBB\xBF\xE2\x80\xA2!";  // BOM
        let mut encoding = Encoding::Unknown;
        let mut buf = [0; 4];
        let mut pos = 0;
        assert_eq!(read_char_from(&mut bytes, &mut encoding, &mut buf, &mut pos).unwrap(), Some('•'));

        let mut bytes: &[u8] = b"\xEF\xBB\xBFx123";  // BOM
        let mut encoding = Encoding::Unknown;
        let mut buf = [0; 4];
        let mut pos = 0;
        assert_eq!(read_char_from(&mut bytes, &mut encoding, &mut buf, &mut pos).unwrap(), Some('x'));

        let mut bytes: &[u8] = b"\xEF\xBB\xBF";  // Nothing after BOM
        let mut encoding = Encoding::Unknown;
        let mut buf = [0; 4];
        let mut pos = 0;
        assert_eq!(read_char_from(&mut bytes, &mut encoding, &mut buf, &mut pos).unwrap(), None);

        let mut bytes: &[u8] = b"\xEF\xBB";  // Nothing after BO
        let mut encoding = Encoding::Unknown;
        let mut buf = [0; 4];
        let mut pos = 0;
        assert!(matches!(read_char_from(&mut bytes, &mut encoding, &mut buf, &mut pos), Err(CharReadError::UnexpectedEof)));

        let mut bytes: &[u8] = b"\xEF\xBB\x42";  // Nothing after BO
        let mut encoding = Encoding::Unknown;
        let mut buf = [0; 4];
        let mut pos = 0;
        assert!(read_char_from(&mut bytes, &mut encoding, &mut buf, &mut pos).is_err());

        let mut bytes: &[u8] = b"\xFE\xFF\x00\x42";  // UTF-16
        let mut encoding = Encoding::Unknown;
        let mut buf = [0; 4];
        let mut pos = 0;
        assert_eq!(read_char_from(&mut bytes, &mut encoding, &mut buf, &mut pos).unwrap(), Some('B'));

        let mut bytes: &[u8] = b"\xFF\xFE\x42\x00";  // UTF-16
        let mut encoding = Encoding::Unknown;
        let mut buf = [0; 4];
        let mut pos = 0;
        assert_eq!(read_char_from(&mut bytes, &mut encoding, &mut buf, &mut pos).unwrap(), Some('B'));

        let mut bytes: &[u8] = b"\xFF\xFE";  // UTF-16
        let mut encoding = Encoding::Unknown;
        let mut buf = [0; 4];
        let mut pos = 0;
        assert_eq!(read_char_from(&mut bytes, &mut encoding, &mut buf, &mut pos).unwrap(), None);

        let mut bytes: &[u8] = b"\xFF\xFE\x00";  // UTF-16
        let mut encoding = Encoding::Unknown;
        let mut buf = [0; 4];
        let mut pos = 0;
        assert!(matches!(read_char_from(&mut bytes, &mut encoding, &mut buf, &mut pos), Err(CharReadError::UnexpectedEof)));

        let mut bytes: &[u8] = "правильно".as_bytes();  // correct BMP
        let mut encoding = Encoding::Unknown;
        let mut buf = [0; 4];
        let mut pos = 0;
        assert_eq!(read_char_from(&mut bytes, &mut encoding, &mut buf, &mut pos).unwrap(), Some('п'));

        let mut bytes: &[u8] = "правильно".as_bytes();
        let mut encoding = Encoding::Utf16Be;
        let mut buf = [0; 4];
        let mut pos = 0;
        assert_eq!(read_char_from(&mut bytes, &mut encoding, &mut buf, &mut pos).unwrap(), Some('킿'));

        let mut bytes: &[u8] = "правильно".as_bytes();
        let mut encoding = Encoding::Utf16Le;
        let mut buf = [0; 4];
        let mut pos = 0;
        assert_eq!(read_char_from(&mut bytes, &mut encoding, &mut buf, &mut pos).unwrap(), Some('뿐'));

        let mut bytes: &[u8] = b"\xD8\xD8\x80";
        let mut encoding = Encoding::Utf16;
        let mut buf = [0; 4];
        let mut pos = 0;
        assert!(read_char_from(&mut bytes, &mut encoding, &mut buf, &mut pos).is_err());

        let mut bytes: &[u8] = b"\x00\x42";
        let mut encoding = Encoding::Utf16;
        let mut buf = [0; 4];
        let mut pos = 0;
        assert_eq!(read_char_from(&mut bytes, &mut encoding, &mut buf, &mut pos).unwrap(), Some('B'));

        let mut bytes: &[u8] = b"\x42\x00";
        let mut encoding = Encoding::Utf16;
        let mut buf = [0; 4];
        let mut pos = 0;
        assert_eq!(read_char_from(&mut bytes, &mut encoding, &mut buf, &mut pos).unwrap(), Some('B'));

        let mut bytes: &[u8] = &[0xEF, 0xBB, 0xBF, 0xFF, 0xFF];
        let mut encoding = Encoding::Utf16;
        let mut buf = [0; 4];
        let mut pos = 0;
        assert!(read_char_from(&mut bytes, &mut encoding, &mut buf, &mut pos).is_err());

        let mut bytes: &[u8] = b"\x00";
        let mut encoding = Encoding::Utf16Be;
        let mut buf = [0; 4];
        let mut pos = 0;
        assert!(read_char_from(&mut bytes, &mut encoding, &mut buf, &mut pos).is_err());

        let mut bytes: &[u8] = "😊".as_bytes();          // correct non-BMP
        let mut encoding = Encoding::Unknown;
        let mut buf = [0; 4];
        let mut pos = 0;
        assert_eq!(read_char_from(&mut bytes, &mut encoding, &mut buf, &mut pos).unwrap(), Some('😊'));

        let mut bytes: &[u8] = b"";                     // empty
        let mut encoding = Encoding::Unknown;
        let mut buf = [0; 4];
        let mut pos = 0;
        assert_eq!(read_char_from(&mut bytes, &mut encoding, &mut buf, &mut pos).unwrap(), None);

        let mut bytes: &[u8] = b"\xf0\x9f\x98";         // incomplete code point
        let mut encoding = Encoding::Unknown;
        let mut buf = [0; 4];
        let mut pos = 0;
        match read_char_from(&mut bytes, &mut encoding, &mut buf, &mut pos).unwrap_err() {
            super::CharReadError::UnexpectedEof => {},
            e => panic!("Unexpected result: {e:?}")
        }

        let mut bytes: &[u8] = b"\xff\x9f\x98\x32";     // invalid code point
        let mut encoding = Encoding::Unknown;
        let mut buf = [0; 4];
        let mut pos = 0;
        match read_char_from(&mut bytes, &mut encoding, &mut buf, &mut pos).unwrap_err() {
            super::CharReadError::Utf8(_) => {},
            e => panic!("Unexpected result: {e:?}")
        }

        // error during read
        struct ErrorReader;
        impl io::Read for ErrorReader {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::new(io::ErrorKind::Other, "test error"))
            }
        }

        let mut r = ErrorReader;
        let mut encoding = Encoding::Unknown;
        let mut buf = [0; 4];
        let mut pos = 0;
        match read_char_from(&mut r, &mut encoding, &mut buf, &mut pos).unwrap_err() {
            super::CharReadError::Io(ref e) if e.kind() == io::ErrorKind::Other &&
                                               e.to_string().contains("test error") => {},
            e => panic!("Unexpected result: {e:?}")
        }
    }
}
