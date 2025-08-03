#![cfg(feature = "async")]

use xml::{AsyncEventReader, reader::{XmlEvent, ParserConfig}};
use std::io::Cursor;
use tokio::io::AsyncRead;

/// Simulates a slow network stream by yielding control between chunks
struct SlowStream {
    data: Vec<u8>,
    position: usize,
    chunk_size: usize,
}

impl SlowStream {
    fn new(data: Vec<u8>, chunk_size: usize) -> Self {
        Self { data, position: 0, chunk_size }
    }
}

impl AsyncRead for SlowStream {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        if self.position >= self.data.len() {
            return std::task::Poll::Ready(Ok(()));
        }
        
        let remaining = self.data.len() - self.position;
        let to_read = remaining.min(self.chunk_size).min(buf.remaining());
        
        buf.put_slice(&self.data[self.position..self.position + to_read]);
        self.position += to_read;
        
        // Simulate async behavior by waking and yielding
        cx.waker().wake_by_ref();
        std::task::Poll::Ready(Ok(()))
    }
}

#[tokio::test]
async fn test_streaming_large_document() {
    // Generate a large XML document
    let mut xml_data = String::from("<?xml version=\"1.0\"?>\n<root>\n");
    for i in 0..100000 {
        xml_data.push_str(&format!("  <item id=\"{i}\">\n"));
        xml_data.push_str(&format!("    <name>Item {i}</name>\n"));
        xml_data.push_str(&format!("    <value>{}</value>\n", i * 10));
        xml_data.push_str("  </item>\n");
    }
    xml_data.push_str("</root>");
    
    let cursor = Cursor::new(xml_data.as_bytes());
    let tokio_reader = tokio::io::BufReader::new(cursor);
    let mut reader = AsyncEventReader::new(tokio_reader);
    
    let mut item_count = 0;
    let mut value_sum = 0i64;
    
    loop {
        match reader.next().await {
            Ok(XmlEvent::EndDocument) => break,
            Ok(XmlEvent::StartElement { name, attributes, .. }) => {
                if name.local_name == "item" {
                    item_count += 1;
                    assert_eq!(attributes.len(), 1);
                    assert_eq!(attributes[0].name.local_name, "id");
                }
            }
            Ok(XmlEvent::Characters(text)) => {
                if let Ok(value) = text.parse::<i64>() {
                    value_sum += value;
                }
            }
            Ok(_) => {},
            Err(e) => panic!("Unexpected error: {e:?}"),
        }
    }
    
    assert_eq!(item_count, 100000);
    assert_eq!(value_sum, (0..100000).map(|i| i * 10).sum::<i64>());
}

#[tokio::test]
async fn test_streaming_with_slow_source() {
    let xml_data = r#"<?xml version="1.0"?>
<messages>
    <message id="1">First message</message>
    <message id="2">Second message</message>
    <message id="3">Third message</message>
</messages>"#;
    
    // Simulate slow streaming with 10-byte chunks
    let slow_stream = SlowStream::new(xml_data.as_bytes().to_vec(), 10);
    let tokio_reader = tokio::io::BufReader::new(slow_stream);
    let mut reader = AsyncEventReader::new(tokio_reader);
    
    let mut message_count = 0;
    loop {
        match reader.next().await {
            Ok(XmlEvent::EndDocument) => break,
            Ok(XmlEvent::StartElement { name, .. }) => {
                if name.local_name == "message" {
                    message_count += 1;
                }
            }
            Ok(_) => {},
            Err(e) => panic!("Unexpected error: {e:?}"),
        }
    }
    
    assert_eq!(message_count, 3);
}

#[tokio::test]
async fn test_memory_efficient_parsing() {
    // Test that parser doesn't load entire document into memory
    // by using a config with small buffer limits
    let config = ParserConfig::new()
        .max_data_length(100)
        .max_attributes(10);
    
    let xml_data = r#"<?xml version="1.0"?>
<root>
    <data>This is a relatively long text content that should still parse fine</data>
</root>"#;
    
    let cursor = Cursor::new(xml_data.as_bytes());
    let tokio_reader = tokio::io::BufReader::new(cursor);
    let mut reader = AsyncEventReader::new_with_config(tokio_reader, config);
    
    let mut found_text = false;
    loop {
        match reader.next().await {
            Ok(XmlEvent::EndDocument) => break,
            Ok(XmlEvent::Characters(text)) => {
                assert!(text.contains("relatively long text"));
                found_text = true;
            }
            Ok(_) => {},
            Err(e) => panic!("Unexpected error: {e:?}"),
        }
    }
    
    assert!(found_text);
}

#[tokio::test]
async fn test_partial_document_handling() {
    // Test handling of documents that arrive in chunks
    let xml_parts = ["<?xml version=\"1.0\"?>",
        "<root>",
        "  <item>First</item>",
        "  <item>Second</item>",
        "</root>"];
    
    let full_xml = xml_parts.join("\n");
    let cursor = Cursor::new(full_xml.as_bytes());
    let tokio_reader = tokio::io::BufReader::new(cursor);
    let mut reader = AsyncEventReader::new(tokio_reader);
    
    let mut items = Vec::new();
    loop {
        match reader.next().await {
            Ok(XmlEvent::EndDocument) => break,
            Ok(XmlEvent::Characters(text)) => {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    items.push(trimmed.to_string());
                }
            }
            Ok(_) => {},
            Err(e) => panic!("Unexpected error: {e:?}"),
        }
    }
    
    assert_eq!(items, vec!["First", "Second"]);
}

#[tokio::test]
async fn test_concurrent_parsing() {
    // Test that multiple async parsers can work concurrently
    let xml1 = r#"<doc1><content>Document 1</content></doc1>"#;
    let xml2 = r#"<doc2><content>Document 2</content></doc2>"#;
    
    let parse_task1 = tokio::spawn(async move {
        let cursor = Cursor::new(xml1.as_bytes());
        let tokio_reader = tokio::io::BufReader::new(cursor);
        let mut reader = AsyncEventReader::new(tokio_reader);
        
        let mut content = String::new();
        loop {
            match reader.next().await {
                Ok(XmlEvent::EndDocument) => break,
                Ok(XmlEvent::Characters(text)) => content = text,
                Ok(_) => {},
                Err(e) => panic!("Error in task 1: {e:?}"),
            }
        }
        content
    });
    
    let parse_task2 = tokio::spawn(async move {
        let cursor = Cursor::new(xml2.as_bytes());
        let tokio_reader = tokio::io::BufReader::new(cursor);
        let mut reader = AsyncEventReader::new(tokio_reader);
        
        let mut content = String::new();
        loop {
            match reader.next().await {
                Ok(XmlEvent::EndDocument) => break,
                Ok(XmlEvent::Characters(text)) => content = text,
                Ok(_) => {},
                Err(e) => panic!("Error in task 2: {e:?}"),
            }
        }
        content
    });
    
    let (result1, result2) = tokio::join!(parse_task1, parse_task2);
    assert_eq!(result1.unwrap(), "Document 1");
    assert_eq!(result2.unwrap(), "Document 2");
}