//! Contains high-level interface for a pull-based XML parser.
//!
//! The most important type in this module is `EventReader`, which provides an iterator
//! view for events in XML document.

use std::io::Read;
use std::iter::FusedIterator;
use std::result;

use crate::common::{Position, TextPosition};

pub use self::config::ParserConfig;
pub use self::error::{Error, ErrorKind};
pub use self::events::XmlEvent;

// back compat
#[doc(hidden)]
#[deprecated(note = "Merged into ParserConfig")]
pub type ParserConfig2 = ParserConfig;

use self::parser::PullParser;
use self::sync_reader::SyncReader;

mod config;
mod error;
mod events;
mod indexset;
mod lexer;
mod parser;
mod xml_read;
mod sync_reader;
#[cfg(feature = "async")]
mod async_reader;

/// A result type yielded by `XmlReader`.
pub type Result<T, E = Error> = result::Result<T, E>;

/// A wrapper around an `std::io::Read` instance which provides pull-based XML parsing.
///
/// The reader should be wrapped in a `BufReader`, otherwise parsing may be very slow.
pub struct EventReader<R: Read> {
    parser: PullParser<SyncReader<R>>,
}

impl<R: Read> EventReader<R> {
    /// Creates a new reader, consuming the given stream. The reader should be wrapped in a `BufReader`, otherwise parsing may be very slow.
    #[inline]
    pub fn new(source: R) -> Self {
        Self::new_with_config(source, ParserConfig::new())
    }

    /// Creates a new reader with the provded configuration, consuming the given stream. The reader should be wrapped in a `BufReader`, otherwise parsing may be very slow.
    #[inline]
    pub fn new_with_config(source: R, config: impl Into<ParserConfig>) -> Self {
        let sync_reader = SyncReader::new(source);
        Self {
            parser: PullParser::new(sync_reader, config),
        }
    }

    /// Pulls and returns next XML event from the stream.
    ///
    /// If this returns [Err] or [`XmlEvent::EndDocument`] then further calls to
    /// this method will return this event again.
    #[inline]
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Result<XmlEvent> {
        self.parser.next()
    }

    /// Skips all XML events until the next end tag at the current level.
    ///
    /// Convenience function that is useful for the case where you have
    /// encountered a start tag that is of no interest and want to
    /// skip the entire XML subtree until the corresponding end tag.
    #[inline]
    pub fn skip(&mut self) -> Result<()> {
        let mut depth = 1;

        while depth > 0 {
            match self.next()? {
                XmlEvent::StartElement { .. } => depth += 1,
                XmlEvent::EndElement { .. } => depth -= 1,
                XmlEvent::EndDocument => return Err(Error {
                    kind: ErrorKind::UnexpectedEof,
                    pos: self.parser.position(),
                }),
                _ => {},
            }
        }

        Ok(())
    }

    /// Access underlying reader
    ///
    /// Using it directly while the event reader is parsing is not recommended
    pub fn source(&self) -> &R { 
        self.parser.reader().get_ref()
    }

    /// Access underlying reader
    ///
    /// Using it directly while the event reader is parsing is not recommended
    pub fn source_mut(&mut self) -> &mut R { 
        self.parser.reader_mut().get_mut()
    }

    /// Unwraps this `EventReader`, returning the underlying reader.
    ///
    /// Note that this operation is destructive; unwrapping the reader and wrapping it
    /// again with `EventReader::new()` will create a fresh reader which will attempt
    /// to parse an XML document from the beginning.
    pub fn into_inner(self) -> R {
        self.parser.into_inner_reader().into_inner()
    }

    /// Returns the DOCTYPE of the document if it has already been seen
    ///
    /// Available only after the root `StartElement` event
    #[inline]
    #[deprecated(note = "there is `XmlEvent::Doctype` now")]
    #[allow(deprecated)]
    pub fn doctype(&self) -> Option<&str> {
        self.parser.doctype()
    }
}

impl<B: Read> Position for EventReader<B> {
    /// Returns the position of the last event produced by the reader.
    #[inline]
    fn position(&self) -> TextPosition {
        self.parser.position()
    }
}

impl<R: Read> IntoIterator for EventReader<R> {
    type IntoIter = Events<R>;
    type Item = Result<XmlEvent>;

    fn into_iter(self) -> Events<R> {
        Events { reader: self, finished: false }
    }
}

/// Async version of EventReader
#[cfg(feature = "async")]
pub struct AsyncEventReader<R: tokio::io::AsyncRead + Unpin + Send> {
    parser: PullParser<async_reader::AsyncReader<R>>,
}

#[cfg(feature = "async")]
impl<R: tokio::io::AsyncRead + Unpin + Send> AsyncEventReader<R> {
    /// Creates a new async reader
    #[inline]
    pub fn new(source: R) -> Self {
        Self::new_with_config(source, ParserConfig::new())
    }

    /// Creates a new async reader with the provided configuration
    #[inline]
    pub fn new_with_config(source: R, config: impl Into<ParserConfig>) -> Self {
        let async_reader = async_reader::AsyncReader::new(source);
        Self {
            parser: PullParser::new_async(async_reader, config),
        }
    }

    /// Pulls and returns next XML event from the stream asynchronously
    #[inline]
    pub async fn next(&mut self) -> Result<XmlEvent> {
        self.parser.next_async().await
    }

    /// Unwraps this `AsyncEventReader`, returning the underlying reader
    pub fn into_inner(self) -> R {
        self.parser.into_inner_reader_async().into_inner()
    }
}

/// An iterator over XML events created from some type implementing `Read`.
///
/// When the next event is `xml::event::Error` or `xml::event::EndDocument`, then
/// it will be returned by the iterator once, and then it will stop producing events.
pub struct Events<R: Read> {
    reader: EventReader<R>,
    finished: bool,
}

impl<R: Read> Events<R> {
    /// Unwraps the iterator, returning the internal `EventReader`.
    #[inline]
    pub fn into_inner(self) -> EventReader<R> {
        self.reader
    }

    /// Access the underlying reader
    ///
    /// It's not recommended to use it while the events are still being parsed
    pub fn source(&self) -> &R { self.reader.source() }

    /// Access the underlying reader
    ///
    /// It's not recommended to use it while the events are still being parsed
    pub fn source_mut(&mut self) -> &mut R { self.reader.source_mut() }
}

impl<R: Read> FusedIterator for Events<R> {
}

impl<R: Read> Iterator for Events<R> {
    type Item = Result<XmlEvent>;

    #[inline]
    fn next(&mut self) -> Option<Result<XmlEvent>> {
        if self.finished && !self.reader.parser.is_ignoring_end_of_stream() {
            None
        } else {
            let ev = self.reader.next();
            if let Ok(XmlEvent::EndDocument) | Err(_) = ev {
                self.finished = true;
            }
            Some(ev)
        }
    }
}

impl<'r> EventReader<&'r [u8]> {
    /// A convenience method to create an `XmlReader` from a string slice.
    #[inline]
    #[must_use]
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(source: &'r str) -> Self {
        EventReader::new(source.as_bytes())
    }
}
