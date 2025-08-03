//! Synchronous reader adapter

use std::io::Read;
use crate::util::{CharReadError, Encoding, read_char_from};
use super::xml_read::XmlRead;

/// Adapter for synchronous `std::io::Read` types
pub struct SyncReader<R: Read> {
    inner: R,
    encoding: Encoding,
    buf: [u8; 4],
    pos: usize,
}

impl<R: Read> SyncReader<R> {
    pub fn new(reader: R) -> Self {
        Self {
            inner: reader,
            encoding: Encoding::Unknown,
            buf: [0; 4],
            pos: 0,
        }
    }
    
    pub fn into_inner(self) -> R {
        self.inner
    }
    
    pub fn get_ref(&self) -> &R {
        &self.inner
    }
    
    pub fn get_mut(&mut self) -> &mut R {
        &mut self.inner
    }
}

impl<R: Read> XmlRead for SyncReader<R> {
    fn read_char(&mut self) -> Result<Option<char>, CharReadError> {
        // Reset position for new character
        self.pos = 0;
        read_char_from(&mut self.inner, &mut self.encoding, &mut self.buf, &mut self.pos)
    }
    
    fn encoding(&self) -> Encoding {
        self.encoding
    }
    
    fn set_encoding(&mut self, encoding: Encoding) {
        self.encoding = encoding;
    }
}