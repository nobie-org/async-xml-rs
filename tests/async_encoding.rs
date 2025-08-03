#![cfg(feature = "async")]

use xml::{AsyncEventReader, reader::XmlEvent};
use std::io::Cursor;

#[tokio::test]
async fn test_utf8_encoding() {
    let xml_data = r#"<?xml version="1.0" encoding="UTF-8"?>
<root>
    <text>Hello, 世界! 🌍</text>
    <emoji>🎉🎊🎈</emoji>
</root>"#;
    
    let cursor = Cursor::new(xml_data.as_bytes());
    let tokio_reader = tokio::io::BufReader::new(cursor);
    let mut reader = AsyncEventReader::new(tokio_reader);
    
    let mut found_unicode = false;
    let mut found_emoji = false;
    
    loop {
        match reader.next().await {
            Ok(XmlEvent::EndDocument) => break,
            Ok(XmlEvent::Characters(text)) => {
                if text.contains("世界") {
                    assert_eq!(text, "Hello, 世界! 🌍");
                    found_unicode = true;
                } else if text.contains("🎉") {
                    assert_eq!(text, "🎉🎊🎈");
                    found_emoji = true;
                }
            }
            Ok(_) => {},
            Err(e) => panic!("Unexpected error: {e:?}"),
        }
    }
    
    assert!(found_unicode, "Unicode text not found");
    assert!(found_emoji, "Emoji text not found");
}

#[tokio::test]
async fn test_utf16_le_encoding() {
    // UTF-16 LE BOM followed by XML declaration
    let xml_data = b"\xFF\xFE<\x00?\x00x\x00m\x00l\x00 \x00v\x00e\x00r\x00s\x00i\x00o\x00n\x00=\x00\"\x001\x00.\x000\x00\"\x00?\x00>\x00<\x00r\x00o\x00o\x00t\x00>\x00T\x00e\x00s\x00t\x00<\x00/\x00r\x00o\x00o\x00t\x00>\x00";
    
    let cursor = Cursor::new(xml_data);
    let tokio_reader = tokio::io::BufReader::new(cursor);
    let mut reader = AsyncEventReader::new(tokio_reader);
    
    let mut found_text = false;
    loop {
        match reader.next().await {
            Ok(XmlEvent::EndDocument) => break,
            Ok(XmlEvent::Characters(text)) => {
                assert_eq!(text, "Test");
                found_text = true;
            }
            Ok(_) => {},
            Err(e) => panic!("Unexpected error: {e:?}"),
        }
    }
    
    assert!(found_text, "Text content not found in UTF-16 LE");
}

#[tokio::test]
async fn test_utf16_be_encoding() {
    // UTF-16 BE BOM followed by XML declaration
    let xml_data = b"\xFE\xFF\x00<\x00?\x00x\x00m\x00l\x00 \x00v\x00e\x00r\x00s\x00i\x00o\x00n\x00=\x00\"\x001\x00.\x000\x00\"\x00?\x00>\x00<\x00r\x00o\x00o\x00t\x00>\x00T\x00e\x00s\x00t\x00<\x00/\x00r\x00o\x00o\x00t\x00>";
    
    let cursor = Cursor::new(xml_data);
    let tokio_reader = tokio::io::BufReader::new(cursor);
    let mut reader = AsyncEventReader::new(tokio_reader);
    
    let mut found_text = false;
    loop {
        match reader.next().await {
            Ok(XmlEvent::EndDocument) => break,
            Ok(XmlEvent::Characters(text)) => {
                assert_eq!(text, "Test");
                found_text = true;
            }
            Ok(_) => {},
            Err(e) => panic!("Unexpected error: {e:?}"),
        }
    }
    
    assert!(found_text, "Text content not found in UTF-16 BE");
}

#[tokio::test]
async fn test_latin1_encoding() {
    // XML with ISO-8859-1 encoding declaration
    let xml_data = b"<?xml version=\"1.0\" encoding=\"ISO-8859-1\"?><root>Caf\xe9</root>";
    
    let cursor = Cursor::new(xml_data);
    let tokio_reader = tokio::io::BufReader::new(cursor);
    let mut reader = AsyncEventReader::new(tokio_reader);
    
    let mut found_text = false;
    loop {
        match reader.next().await {
            Ok(XmlEvent::EndDocument) => break,
            Ok(XmlEvent::StartDocument { encoding, .. }) => {
                assert_eq!(encoding, "ISO-8859-1");
            }
            Ok(XmlEvent::Characters(text)) => {
                assert_eq!(text, "Café");
                found_text = true;
            }
            Ok(_) => {},
            Err(e) => panic!("Unexpected error: {e:?}"),
        }
    }
    
    assert!(found_text, "Latin-1 encoded text not found");
}

#[tokio::test]
async fn test_bom_detection() {
    // UTF-8 with BOM
    let xml_data_utf8_bom = b"\xEF\xBB\xBF<?xml version=\"1.0\"?><root>UTF-8 BOM</root>";
    
    let cursor = Cursor::new(xml_data_utf8_bom);
    let tokio_reader = tokio::io::BufReader::new(cursor);
    let mut reader = AsyncEventReader::new(tokio_reader);
    
    let mut found_utf8_bom = false;
    loop {
        match reader.next().await {
            Ok(XmlEvent::EndDocument) => break,
            Ok(XmlEvent::Characters(text)) => {
                assert_eq!(text, "UTF-8 BOM");
                found_utf8_bom = true;
            }
            Ok(_) => {},
            Err(e) => panic!("Unexpected error: {e:?}"),
        }
    }
    
    assert!(found_utf8_bom, "UTF-8 BOM text not found");
}

#[tokio::test]
async fn test_encoding_mismatch_detection() {
    // Declare UTF-8 but use invalid UTF-8 sequence
    let xml_data = b"<?xml version=\"1.0\" encoding=\"UTF-8\"?><root>\xFF\xFE</root>";
    
    let cursor = Cursor::new(xml_data);
    let tokio_reader = tokio::io::BufReader::new(cursor);
    let mut reader = AsyncEventReader::new(tokio_reader);
    
    let mut found_error = false;
    loop {
        match reader.next().await {
            Ok(XmlEvent::EndDocument) => break,
            Ok(_) => continue,
            Err(_) => {
                found_error = true;
                break;
            }
        }
    }
    
    assert!(found_error, "Expected error for invalid UTF-8");
}

#[tokio::test]
async fn test_windows_1252_encoding() {
    // Test that unsupported encodings are handled gracefully
    let xml_data = b"<?xml version=\"1.0\" encoding=\"Windows-1252\"?><root>Windows</root>";
    
    let cursor = Cursor::new(xml_data);
    let tokio_reader = tokio::io::BufReader::new(cursor);
    let mut reader = AsyncEventReader::new(tokio_reader);
    
    let mut found_error = false;
    loop {
        match reader.next().await {
            Ok(XmlEvent::EndDocument) => break,
            Ok(_) => continue,
            Err(e) => {
                let error_str = format!("{e:?}");
                assert!(error_str.contains("Windows-1252") || error_str.contains("Unsupported"));
                found_error = true;
                break;
            }
        }
    }
    
    assert!(found_error, "Expected error for unsupported Windows-1252 encoding");
}