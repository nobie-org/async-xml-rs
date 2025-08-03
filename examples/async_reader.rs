//! Example of using the async XML reader

#[cfg(feature = "async")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use xml::{AsyncEventReader, reader::XmlEvent};
    
    let xml_data = r#"<?xml version="1.0" encoding="UTF-8"?>
<root>
    <child attr="value">Text content</child>
    <child>Another child</child>
</root>"#;
    
    // Create an async reader from the string data
    let cursor = std::io::Cursor::new(xml_data.as_bytes());
    let tokio_reader = tokio::io::BufReader::new(cursor);
    let mut reader = AsyncEventReader::new(tokio_reader);
    
    loop {
        match reader.next().await? {
            XmlEvent::StartDocument { .. } => {
                println!("Start of document");
            }
            XmlEvent::StartElement { name, attributes, .. } => {
                println!("Start element: {name}");
                for attr in attributes {
                    println!("  Attribute: {} = {}", attr.name, attr.value);
                }
            }
            XmlEvent::Characters(text) => {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    println!("Text: {trimmed}");
                }
            }
            XmlEvent::EndElement { name } => {
                println!("End element: {name}");
            }
            XmlEvent::EndDocument => {
                println!("End of document");
                break;
            }
            _ => {}
        }
    }
    
    Ok(())
}

#[cfg(not(feature = "async"))]
fn main() {
    println!("This example requires the 'async' feature to be enabled.");
    println!("Run with: cargo run --example async_reader --features async");
}