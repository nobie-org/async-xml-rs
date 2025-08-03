//! Asynchronous reader adapter

use tokio::io::AsyncRead;
use crate::{reader::Error, util::{CharReadError, Encoding}};
use super::xml_read::{AsyncXmlRead, XmlRead};

/// Adapter for asynchronous `tokio::io::AsyncRead` types
pub struct AsyncReader<R: AsyncRead + Unpin + Send> {
    inner: R,
    encoding: Encoding,
    buf: [u8; 4],
    pos: usize,
}

impl<R: AsyncRead + Unpin + Send> AsyncReader<R> {
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
    
    #[allow(dead_code)]
    pub fn get_ref(&self) -> &R {
        &self.inner
    }
    
    #[allow(dead_code)]
    pub fn get_mut(&mut self) -> &mut R {
        &mut self.inner
    }
}

impl<R: AsyncRead + Unpin + Send> AsyncXmlRead for AsyncReader<R> {
    async fn read_char(&mut self) -> Result<Option<char>, Error> {
        // Reset position for new character
        self.pos = 0;
        Ok(crate::util::async_read_char_from(&mut self.inner, &mut self.encoding, &mut self.buf, &mut self.pos).await?)
    }
    
    fn encoding(&self) -> Encoding {
        self.encoding
    }
    
    fn set_encoding(&mut self, encoding: Encoding) {
        self.encoding = encoding;
    }
}

impl<R: AsyncRead + Unpin + Send> XmlRead for AsyncReader<R> {
    fn read_char(&mut self) -> Result<Option<char>, CharReadError> {
        // AsyncReader should not be used in synchronous contexts
        // This implementation exists only to satisfy the trait bound
        Err(CharReadError::Io(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "AsyncReader cannot be used in synchronous contexts. Use AsyncXmlRead::read_char instead."
        )))
    }
    
    fn encoding(&self) -> Encoding {
        self.encoding
    }
    
    fn set_encoding(&mut self, encoding: Encoding) {
        self.encoding = encoding;
    }
}