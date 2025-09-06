#![cfg(feature = "async")]

use xml::{AsyncEventReader, reader::XmlEvent};
use std::io::Cursor;

#[tokio::test]
async fn test_basic_parsing() {
    let xml_data = r#"<?xml version="1.0" encoding="UTF-8"?>
<root>
    <child attr="value">Text content</child>
    <child>Another child</child>
</root>"#;
    
    let cursor = Cursor::new(xml_data.as_bytes());
    let tokio_reader = tokio::io::BufReader::new(cursor);
    let mut reader = AsyncEventReader::new(tokio_reader);
    
    let mut events = Vec::new();
    loop {
        match reader.next().await {
            Ok(XmlEvent::EndDocument) => {
                events.push(XmlEvent::EndDocument);
                break;
            }
            Ok(event) => events.push(event),
            Err(e) => panic!("Unexpected error: {e:?}"),
        }
    }
    
    // Filter out whitespace events for easier testing
    let non_whitespace_events: Vec<_> = events.into_iter()
        .filter(|e| !matches!(e, XmlEvent::Whitespace(_)))
        .collect();
    
    assert!(non_whitespace_events.len() >= 7);
    
    // Verify the sequence of events
    match &non_whitespace_events[0] {
        XmlEvent::StartDocument { version, encoding, .. } => {
            assert_eq!(version, &xml::common::XmlVersion::Version10);
            assert_eq!(encoding, "UTF-8");
        }
        _ => panic!("Expected StartDocument"),
    }
    
    match &non_whitespace_events[1] {
        XmlEvent::StartElement { name, .. } => {
            assert_eq!(name.local_name, "root");
        }
        _ => panic!("Expected StartElement for root"),
    }
    
    match &non_whitespace_events[2] {
        XmlEvent::StartElement { name, attributes, .. } => {
            assert_eq!(name.local_name, "child");
            assert_eq!(attributes.len(), 1);
            assert_eq!(attributes[0].name.local_name, "attr");
            assert_eq!(&attributes[0].value, "value");
        }
        _ => panic!("Expected StartElement for child"),
    }
    
    match &non_whitespace_events[3] {
        XmlEvent::Characters(text) => {
            assert_eq!(text, "Text content");
        }
        _ => panic!("Expected Characters"),
    }
}

#[tokio::test]
async fn test_empty_element() {
    let xml_data = r#"<root><empty/></root>"#;
    
    let cursor = Cursor::new(xml_data.as_bytes());
    let tokio_reader = tokio::io::BufReader::new(cursor);
    let mut reader = AsyncEventReader::new(tokio_reader);
    
    let mut events = Vec::new();
    loop {
        match reader.next().await {
            Ok(XmlEvent::EndDocument) => break,
            Ok(event) => events.push(event),
            Err(e) => panic!("Unexpected error: {e:?}"),
        }
    }
    
    // Should have: StartDocument, StartElement(root), StartElement(empty), EndElement(empty), EndElement(root)
    assert_eq!(events.len(), 5);
}

#[tokio::test]
async fn test_comments_and_cdata() {
    let xml_data = r#"<?xml version="1.0"?>
<root>
    <!-- This is a comment -->
    <data><![CDATA[Some <text> with & special characters]]></data>
</root>"#;
    
    let cursor = Cursor::new(xml_data.as_bytes());
    let tokio_reader = tokio::io::BufReader::new(cursor);
    let mut reader = AsyncEventReader::new(tokio_reader);
    
    let mut found_cdata = false;
    loop {
        match reader.next().await {
            Ok(XmlEvent::EndDocument) => break,
            Ok(XmlEvent::CData(text)) => {
                assert_eq!(text, "Some <text> with & special characters");
                found_cdata = true;
            }
            Ok(_) => {},
            Err(e) => panic!("Unexpected error: {e:?}"),
        }
    }
    
    assert!(found_cdata, "CDATA section was not found");
}

#[tokio::test]
async fn test_namespaces() {
    let xml_data = r#"<?xml version="1.0"?>
<root xmlns="http://example.com/ns" xmlns:custom="http://example.com/custom">
    <child/>
    <custom:element/>
</root>"#;
    
    let cursor = Cursor::new(xml_data.as_bytes());
    let tokio_reader = tokio::io::BufReader::new(cursor);
    let mut reader = AsyncEventReader::new(tokio_reader);
    
    let mut namespace_events = 0;
    loop {
        match reader.next().await {
            Ok(XmlEvent::EndDocument) => break,
            Ok(XmlEvent::StartElement { name, namespace, .. }) => {
                if name.namespace.is_some() {
                    namespace_events += 1;
                }
                if name.prefix.is_some() {
                    assert_eq!(name.prefix.as_ref().unwrap(), "custom");
                }
                // Check namespace mappings
                if name.local_name == "root" {
                    assert!(namespace.get("") == Some("http://example.com/ns"));
                    assert!(namespace.get("custom") == Some("http://example.com/custom"));
                }
            }
            Ok(_) => {},
            Err(e) => panic!("Unexpected error: {e:?}"),
        }
    }
    
    assert!(namespace_events > 0, "No namespaced elements found");
}

#[tokio::test]
async fn test_processing_instructions() {
    let xml_data = r#"<?xml version="1.0"?>
<?xml-stylesheet type="text/xsl" href="style.xsl"?>
<root>
    <?process some data?>
</root>"#;
    
    let cursor = Cursor::new(xml_data.as_bytes());
    let tokio_reader = tokio::io::BufReader::new(cursor);
    let mut reader = AsyncEventReader::new(tokio_reader);
    
    let mut pi_count = 0;
    loop {
        match reader.next().await {
            Ok(XmlEvent::EndDocument) => break,
            Ok(XmlEvent::ProcessingInstruction { name, data }) => {
                pi_count += 1;
                if name == "xml-stylesheet" {
                    assert!(data.is_some());
                    assert!(data.unwrap().contains("style.xsl"));
                } else if name == "process" {
                    assert_eq!(data, Some("some data".to_string()));
                }
            }
            Ok(_) => {},
            Err(e) => panic!("Unexpected error: {e:?}"),
        }
    }
    
    assert_eq!(pi_count, 2, "Expected 2 processing instructions");
}

#[tokio::test]
async fn test_whitespace_handling() {
    use xml::reader::ParserConfig;
    
    let xml_data = r#"<root>
    <child>   Text with spaces   </child>
</root>"#;
    
    let cursor = Cursor::new(xml_data.as_bytes());
    let tokio_reader = tokio::io::BufReader::new(cursor);
    
    // Test with whitespace trimming
    let config = ParserConfig::new().trim_whitespace(true);
    let mut reader = AsyncEventReader::new_with_config(tokio_reader, config);
    
    let mut found_text = false;
    loop {
        match reader.next().await {
            Ok(XmlEvent::EndDocument) => break,
            Ok(XmlEvent::Characters(text)) => {
                assert_eq!(text, "Text with spaces");
                found_text = true;
            }
            Ok(_) => {},
            Err(e) => panic!("Unexpected error: {e:?}"),
        }
    }
    
    assert!(found_text, "Text content not found");
}

#[tokio::test]
async fn test_attributes_with_entities() {
    let xml_data = r#"<root attr="&lt;value&gt;" attr2="&quot;quoted&quot;"/>"#;
    
    let cursor = Cursor::new(xml_data.as_bytes());
    let tokio_reader = tokio::io::BufReader::new(cursor);
    let mut reader = AsyncEventReader::new(tokio_reader);
    
    loop {
        match reader.next().await {
            Ok(XmlEvent::EndDocument) => break,
            Ok(XmlEvent::StartElement { attributes, .. }) => {
                if !attributes.is_empty() {
                    assert_eq!(attributes.len(), 2);
                    assert_eq!(&attributes[0].value, "<value>");
                    assert_eq!(&attributes[1].value, "\"quoted\"");
                }
            }
            Ok(_) => {},
            Err(e) => panic!("Unexpected error: {e:?}"),
        }
    }
}

#[tokio::test] 
async fn test_doctype() {
    let xml_data = r#"<?xml version="1.0"?>
<!DOCTYPE root SYSTEM "root.dtd">
<root/>"#;
    
    let cursor = Cursor::new(xml_data.as_bytes());
    let tokio_reader = tokio::io::BufReader::new(cursor);
    let mut reader = AsyncEventReader::new(tokio_reader);
    
    let mut found_doctype = false;
    loop {
        match reader.next().await {
            Ok(XmlEvent::EndDocument) => break,
            Ok(XmlEvent::Doctype { syntax }) => {
                assert!(syntax.contains("DOCTYPE"));
                assert!(syntax.contains("root"));
                assert!(syntax.contains("SYSTEM"));
                found_doctype = true;
            }
            Ok(_) => {},
            Err(e) => panic!("Unexpected error: {e:?}"),
        }
    }
    
    assert!(found_doctype, "DOCTYPE declaration not found");
}