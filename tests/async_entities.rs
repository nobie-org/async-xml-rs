#![cfg(feature = "async")]

use xml::{AsyncEventReader, reader::{XmlEvent, ParserConfig}};
use std::io::Cursor;
use std::collections::HashMap;

#[tokio::test]
async fn test_predefined_entities() {
    let xml_data = r#"<?xml version="1.0"?>
<root>
    <less>&lt;</less>
    <greater>&gt;</greater>
    <amp>&amp;</amp>
    <apos>&apos;</apos>
    <quot>&quot;</quot>
    <all>&lt;&gt;&amp;&apos;&quot;</all>
</root>"#;
    
    let cursor = Cursor::new(xml_data.as_bytes());
    let tokio_reader = tokio::io::BufReader::new(cursor);
    let mut reader = AsyncEventReader::new(tokio_reader);
    
    let mut entities_found = HashMap::new();
    let mut current_element = String::new();
    
    loop {
        match reader.next().await {
            Ok(XmlEvent::EndDocument) => break,
            Ok(XmlEvent::StartElement { name, .. }) => {
                current_element = name.local_name.clone();
            }
            Ok(XmlEvent::Characters(text)) => {
                entities_found.insert(current_element.clone(), text);
            }
            Ok(_) => {},
            Err(e) => panic!("Unexpected error: {e:?}"),
        }
    }
    
    assert_eq!(entities_found.get("less").unwrap(), "<");
    assert_eq!(entities_found.get("greater").unwrap(), ">");
    assert_eq!(entities_found.get("amp").unwrap(), "&");
    assert_eq!(entities_found.get("apos").unwrap(), "'");
    assert_eq!(entities_found.get("quot").unwrap(), "\"");
    assert_eq!(entities_found.get("all").unwrap(), "<>&'\"");
}

#[tokio::test]
async fn test_numeric_character_entities() {
    let xml_data = r#"<?xml version="1.0"?>
<root>
    <decimal>&#65;&#66;&#67;</decimal>
    <hex>&#x41;&#x42;&#x43;</hex>
    <unicode>&#x1F600;&#x1F601;</unicode>
    <mixed>A&#66;C&#x44;E</mixed>
</root>"#;
    
    let cursor = Cursor::new(xml_data.as_bytes());
    let tokio_reader = tokio::io::BufReader::new(cursor);
    let mut reader = AsyncEventReader::new(tokio_reader);
    
    let mut entities_found = HashMap::new();
    let mut current_element = String::new();
    
    loop {
        match reader.next().await {
            Ok(XmlEvent::EndDocument) => break,
            Ok(XmlEvent::StartElement { name, .. }) => {
                current_element = name.local_name.clone();
            }
            Ok(XmlEvent::Characters(text)) => {
                entities_found.insert(current_element.clone(), text);
            }
            Ok(_) => {},
            Err(e) => panic!("Unexpected error: {e:?}"),
        }
    }
    
    assert_eq!(entities_found.get("decimal").unwrap(), "ABC");
    assert_eq!(entities_found.get("hex").unwrap(), "ABC");
    assert_eq!(entities_found.get("unicode").unwrap(), "😀😁");
    assert_eq!(entities_found.get("mixed").unwrap(), "ABCDE");
}

#[tokio::test]
async fn test_custom_entities() {
    let config = ParserConfig::new()
        .add_entity("custom", "CUSTOM_VALUE")
        .add_entity("nbsp", "\u{00A0}");
    
    let xml_data = r#"<?xml version="1.0"?>
<root>
    <custom>&custom;</custom>
    <space>Hello&nbsp;World</space>
</root>"#;
    
    let cursor = Cursor::new(xml_data.as_bytes());
    let tokio_reader = tokio::io::BufReader::new(cursor);
    let mut reader = AsyncEventReader::new_with_config(tokio_reader, config);
    
    let mut found_custom = false;
    let mut found_nbsp = false;
    
    loop {
        match reader.next().await {
            Ok(XmlEvent::EndDocument) => break,
            Ok(XmlEvent::Characters(text)) => {
                if text == "CUSTOM_VALUE" {
                    found_custom = true;
                } else if text.contains('\u{00A0}') {
                    assert_eq!(text, "Hello\u{00A0}World");
                    found_nbsp = true;
                }
            }
            Ok(_) => {},
            Err(e) => panic!("Unexpected error: {e:?}"),
        }
    }
    
    assert!(found_custom, "Custom entity not resolved");
    assert!(found_nbsp, "Non-breaking space entity not resolved");
}

#[tokio::test]
async fn test_entity_in_attributes() {
    let xml_data = r#"<?xml version="1.0"?>
<root>
    <element attr1="&lt;value&gt;" attr2="&quot;quoted&quot;" attr3="A&#66;C"/>
</root>"#;
    
    let cursor = Cursor::new(xml_data.as_bytes());
    let tokio_reader = tokio::io::BufReader::new(cursor);
    let mut reader = AsyncEventReader::new(tokio_reader);
    
    loop {
        match reader.next().await {
            Ok(XmlEvent::EndDocument) => break,
            Ok(XmlEvent::StartElement { name, attributes, .. }) => {
                if name.local_name == "element" {
                    assert_eq!(attributes.len(), 3);
                    assert_eq!(&attributes[0].value, "<value>");
                    assert_eq!(&attributes[1].value, "\"quoted\"");
                    assert_eq!(&attributes[2].value, "ABC");
                }
            }
            Ok(_) => {},
            Err(e) => panic!("Unexpected error: {e:?}"),
        }
    }
}

#[tokio::test]
async fn test_entity_expansion_limits() {
    // Test that entity expansion doesn't cause issues
    let xml_data = r#"<?xml version="1.0"?>
<!DOCTYPE root [
    <!ENTITY long "This is a long entity value that contains some text">
]>
<root>
    <text>&long; and &long; again</text>
</root>"#;
    
    let cursor = Cursor::new(xml_data.as_bytes());
    let tokio_reader = tokio::io::BufReader::new(cursor);
    let mut reader = AsyncEventReader::new(tokio_reader);
    
    let mut found_expansion = false;
    loop {
        match reader.next().await {
            Ok(XmlEvent::EndDocument) => break,
            Ok(XmlEvent::Characters(text)) => {
                if text.contains("This is a long entity") {
                    assert!(text.contains("again"));
                    found_expansion = true;
                }
            }
            Ok(_) => {},
            Err(e) => panic!("Unexpected error: {e:?}"),
        }
    }
    
    assert!(found_expansion, "Entity expansion not found");
}

#[tokio::test]
async fn test_invalid_numeric_entities() {
    // Test handling of invalid numeric character references
    let xml_data = r#"<?xml version="1.0"?>
<root>
    <invalid>&#xFFFE;</invalid>
</root>"#;
    
    let config = ParserConfig::new()
        .replace_unknown_entity_references(true);
    
    let cursor = Cursor::new(xml_data.as_bytes());
    let tokio_reader = tokio::io::BufReader::new(cursor);
    let mut reader = AsyncEventReader::new_with_config(tokio_reader, config);
    
    let mut found_replacement = false;
    loop {
        match reader.next().await {
            Ok(XmlEvent::EndDocument) => break,
            Ok(XmlEvent::Characters(text)) => {
                // Should be replaced with replacement character
                if text.contains('\u{FFFD}') {
                    found_replacement = true;
                }
            }
            Ok(_) => {},
            Err(e) => panic!("Unexpected error: {e:?}"),
        }
    }
    
    assert!(found_replacement, "Invalid entity should be replaced");
}

#[tokio::test]
async fn test_entity_in_cdata() {
    // Entities should NOT be expanded in CDATA sections
    let xml_data = r#"<?xml version="1.0"?>
<root>
    <cdata><![CDATA[This contains &lt; and &gt; and &#65; literally]]></cdata>
</root>"#;
    
    let cursor = Cursor::new(xml_data.as_bytes());
    let tokio_reader = tokio::io::BufReader::new(cursor);
    let mut reader = AsyncEventReader::new(tokio_reader);
    
    loop {
        match reader.next().await {
            Ok(XmlEvent::EndDocument) => break,
            Ok(XmlEvent::CData(text)) => {
                assert_eq!(text, "This contains &lt; and &gt; and &#65; literally");
            }
            Ok(_) => {},
            Err(e) => panic!("Unexpected error: {e:?}"),
        }
    }
}