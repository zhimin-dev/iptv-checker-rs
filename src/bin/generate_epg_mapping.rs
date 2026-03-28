use quick_xml::events::Event;
use quick_xml::reader::Reader;
use serde::Serialize;
use std::fs::File;
use std::io::Write;

#[derive(Debug, Serialize)]
struct EpgMapping {
    name: String,
    channel: String,
    source: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let xml_path = "/Users/meow.zang/Desktop/epg.xml";
    let output_path = "src/assets/epg_mapping.json";

    println!("Parsing XML from: {}", xml_path);

    let mut reader = Reader::from_file(xml_path)?;
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut mappings = Vec::new();

    let mut in_channel = false;
    let mut current_channel_id = String::new();
    let mut in_display_name = false;
    let mut current_display_name = String::new();
    let mut current_lang = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                match e.name().as_ref() {
                    b"channel" => {
                        in_channel = true;
                        for attr in e.attributes() {
                            let attr = attr?;
                            if attr.key.as_ref() == b"id" {
                                current_channel_id = String::from_utf8_lossy(&attr.value).into_owned();
                            }
                        }
                    }
                    b"display-name" if in_channel => {
                        in_display_name = true;
                        current_display_name.clear();
                        current_lang = String::new();
                        for attr in e.attributes() {
                            let attr = attr?;
                            if attr.key.as_ref() == b"lang" {
                                current_lang = String::from_utf8_lossy(&attr.value).into_owned();
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(e)) => {
                if in_display_name {
                    current_display_name = e.unescape()?.into_owned();
                }
            }
            Ok(Event::End(ref e)) => {
                match e.name().as_ref() {
                    b"channel" => {
                        in_channel = false;
                        current_channel_id.clear();
                    }
                    b"display-name" if in_channel => {
                        in_display_name = false;
                        if !current_display_name.is_empty() && !current_channel_id.is_empty() {
                            // Extract source from channel ID if possible, otherwise use lang or default
                            let source = if current_channel_id.ends_with(".cn") {
                                "cn".to_string()
                            } else if current_channel_id.ends_with(".hk") {
                                "hk".to_string()
                            } else if current_channel_id.ends_with(".tw") {
                                "tw".to_string()
                            } else if current_channel_id.ends_with(".mo") {
                                "mo".to_string()
                            } else if !current_lang.is_empty() {
                                current_lang.clone()
                            } else {
                                "unknown".to_string()
                            };

                            mappings.push(EpgMapping {
                                name: current_display_name.clone(),
                                channel: current_channel_id.clone(),
                                source,
                            });
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                eprintln!("Error at position {}: {:?}", reader.buffer_position(), e);
                break;
            }
            _ => (),
        }
        buf.clear();
    }

    println!("Found {} mappings", mappings.len());

    let json = serde_json::to_string_pretty(&mappings)?;
    let mut file = File::create(output_path)?;
    file.write_all(json.as_bytes())?;

    println!("Saved mappings to: {}", output_path);

    Ok(())
}
