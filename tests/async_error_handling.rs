#![cfg(feature = "async")]

use xml::{AsyncEventReader, reader::XmlEvent};
use std::io::Cursor;

#[tokio::test]
async fn test_unclosed_tag_error() {
    let xml_data = r#"<root><child>Content"#;
    
    let cursor = Cursor::new(xml_data.as_bytes());
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
    
    assert!(found_error, "Expected error for unclosed tag");
}

#[tokio::test]
async fn test_mismatched_tags() {
    let xml_data = r#"<root><child></wrong></root>"#;
    
    let cursor = Cursor::new(xml_data.as_bytes());
    let tokio_reader = tokio::io::BufReader::new(cursor);
    let mut reader = AsyncEventReader::new(tokio_reader);
    
    let mut found_error = false;
    loop {
        match reader.next().await {
            Ok(XmlEvent::EndDocument) => break,
            Ok(_) => continue,
            Err(e) => {
                found_error = true;
                let error_str = format!("{e:?}");
                assert!(error_str.contains("child") || error_str.contains("wrong"));
                break;
            }
        }
    }
    
    assert!(found_error, "Expected error for mismatched tags");
}

#[tokio::test]
async fn test_invalid_xml_declaration() {
    let xml_data = r#"<?xml version="2.0"?><root/>"#;
    
    let cursor = Cursor::new(xml_data.as_bytes());
    let tokio_reader = tokio::io::BufReader::new(cursor);
    let mut reader = AsyncEventReader::new(tokio_reader);
    
    let mut found_error = false;
    loop {
        match reader.next().await {
            Ok(XmlEvent::EndDocument) => break,
            Ok(_) => continue,
            Err(e) => {
                found_error = true;
                let error_str = format!("{e:?}");
                assert!(error_str.contains("version") || error_str.contains("2.0"));
                break;
            }
        }
    }
    
    assert!(found_error, "Expected error for invalid XML version");
}

#[tokio::test]
async fn test_duplicate_attributes() {
    let xml_data = r#"<root attr="value1" attr="value2"/>"#;
    
    let cursor = Cursor::new(xml_data.as_bytes());
    let tokio_reader = tokio::io::BufReader::new(cursor);
    let mut reader = AsyncEventReader::new(tokio_reader);
    
    let mut found_error = false;
    loop {
        match reader.next().await {
            Ok(XmlEvent::EndDocument) => break,
            Ok(_) => continue,
            Err(e) => {
                found_error = true;
                let error_str = format!("{e:?}");
                assert!(error_str.contains("attr") || error_str.contains("duplicate") || error_str.contains("redefined"));
                break;
            }
        }
    }
    
    assert!(found_error, "Expected error for duplicate attributes");
}

#[tokio::test]
async fn test_invalid_entity_reference() {
    let xml_data = r#"<root>&invalid;</root>"#;
    
    let cursor = Cursor::new(xml_data.as_bytes());
    let tokio_reader = tokio::io::BufReader::new(cursor);
    let mut reader = AsyncEventReader::new(tokio_reader);
    
    let mut found_error = false;
    loop {
        match reader.next().await {
            Ok(XmlEvent::EndDocument) => break,
            Ok(_) => continue,
            Err(e) => {
                found_error = true;
                let error_str = format!("{e:?}");
                assert!(error_str.contains("invalid") || error_str.contains("entity"));
                break;
            }
        }
    }
    
    assert!(found_error, "Expected error for invalid entity reference");
}

#[tokio::test]
async fn test_invalid_character_in_tag_name() {
    let xml_data = r#"<root><123invalid/></root>"#;
    
    let cursor = Cursor::new(xml_data.as_bytes());
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
    
    assert!(found_error, "Expected error for invalid tag name");
}

#[tokio::test]
async fn test_text_before_root_element() {
    let xml_data = r#"Some text before root<root/>"#;
    
    let cursor = Cursor::new(xml_data.as_bytes());
    let tokio_reader = tokio::io::BufReader::new(cursor);
    let mut reader = AsyncEventReader::new(tokio_reader);
    
    let mut found_error = false;
    loop {
        match reader.next().await {
            Ok(XmlEvent::EndDocument) => break,
            Ok(_) => continue,
            Err(e) => {
                found_error = true;
                let error_str = format!("{e:?}");
                assert!(error_str.contains("root") || error_str.contains("outside"));
                break;
            }
        }
    }
    
    assert!(found_error, "Expected error for text before root element");
}

#[tokio::test]
async fn test_multiple_root_elements() {
    use xml::reader::ParserConfig;
    
    let xml_data = r#"<root1/><root2/>"#;
    
    let cursor = Cursor::new(xml_data.as_bytes());
    let tokio_reader = tokio::io::BufReader::new(cursor);
    let config = ParserConfig::new().allow_multiple_root_elements(false);
    let mut reader = AsyncEventReader::new_with_config(tokio_reader, config);
    
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
    
    assert!(found_error, "Expected error for multiple root elements");
}

#[tokio::test]
async fn test_invalid_comment() {
    let xml_data = r#"<root><!-- Invalid -- comment --></root>"#;
    
    let cursor = Cursor::new(xml_data.as_bytes());
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
    
    assert!(found_error, "Expected error for invalid comment");
}

#[tokio::test]
async fn test_unclosed_cdata() {
    let xml_data = r#"<root><![CDATA[Unclosed CDATA</root>"#;
    
    let cursor = Cursor::new(xml_data.as_bytes());
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
    
    assert!(found_error, "Expected error for unclosed CDATA");
}