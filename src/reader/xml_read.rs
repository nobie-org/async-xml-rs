//! I/O abstraction traits for XML parsing

#[cfg(feature = "async")]
use crate::reader::Error;
use crate::util::{CharReadError, Encoding};

/// Trait for reading characters from an XML source
pub trait XmlRead {
    /// Read the next character from the source
    fn read_char(&mut self) -> Result<Option<char>, CharReadError>;
    
    /// Get the current encoding
    fn encoding(&self) -> Encoding;
    
    /// Set the encoding
    fn set_encoding(&mut self, encoding: Encoding);
}

/// Trait for asynchronously reading characters from an XML source
#[cfg(feature = "async")]
pub trait AsyncXmlRead {
    /// Read the next character from the source
    fn read_char(&mut self) -> impl std::future::Future<Output = Result<Option<char>, Error>> + Send;
    
    /// Get the current encoding
    #[allow(dead_code)]
    fn encoding(&self) -> Encoding;
    
    /// Set the encoding
    #[allow(dead_code)]
    fn set_encoding(&mut self, encoding: Encoding);
}