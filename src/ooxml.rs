use quick_xml::de::from_reader;
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use quick_xml::se::Serializer;
use quick_xml::{Reader, Writer};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, BufWriter, Cursor, Write};
use std::path::PathBuf;

pub mod schemas {
    pub mod shared_strings {
        use serde::{Deserialize, Serialize};

        #[derive(Serialize, Deserialize, Debug)]
        pub struct Sst {
            #[serde(rename = "@xmlns")]
            pub xmlns: String,
            #[serde(rename = "@count")]
            pub count: String,
            #[serde(rename = "@uniqueCount")]
            pub unique_count: String,
            pub si: Vec<Si>,
        }

        #[derive(Serialize, Deserialize, Debug, Clone)]
        pub struct Si {
            pub t: String,
        }
    }
}

fn read_xml_file<T: DeserializeOwned>(file_path: &str) -> Result<T, Box<dyn std::error::Error>> {
    let file = File::open(file_path)?;
    let reader = BufReader::new(file);
    let result: T = from_reader(reader).unwrap();

    Ok(result)
}

fn _write_xml_file<T: Serialize>(
    file_path: &PathBuf,
    value: &T,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut xml_string = String::new();

    let serializer = Serializer::new(&mut xml_string);
    value.serialize(serializer)?;

    let mut file = File::create(file_path)?;
    file.write_all(&xml_string.into_bytes())?;

    Ok(())
}

pub fn inline_strings(
    shared_strings_file_path: &str,
    target_file_path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let sst: schemas::shared_strings::Sst = read_xml_file(shared_strings_file_path)?;

    let mut reader = Reader::from_file(target_file_path)?;
    reader.config_mut().trim_text(true);

    let mut output = Vec::new();
    let mut writer = Writer::new_with_indent(Cursor::new(&mut output), b' ', 4);
    writer.write_event(Event::Decl(BytesDecl::new(
        "1.0",
        Some("UTF-8"),
        Some("yes"),
    )))?;

    let mut buf = Vec::new();
    let mut in_t_cell = false;
    let mut in_t_cell_value = false;
    let mut cell_ref = None;
    let mut ss_index = None;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => match e.name().as_ref() {
                b"c" => {
                    let attributes = e.attributes().map(|a| a.unwrap()).collect::<Vec<_>>();
                    let attributes = attributes
                        .iter()
                        .map(|a| (a.key.as_ref(), a.unescape_value().unwrap()))
                        .collect::<HashMap<_, _>>();

                    if let Some(cell_type) = attributes.get(b"t".as_ref()) {
                        log::trace!("Cell type: {:?}", cell_type);
                        if cell_type.as_ref() == "s" {
                            in_t_cell = true;
                            cell_ref = Some(attributes.get(b"r".as_ref()).unwrap().to_string());
                        } else {
                            writer.write_event(Event::Start(e.to_owned()))?;
                            log::trace!("Wrote start event: {:?}", e);
                        }
                    } else {
                        writer.write_event(Event::Start(e.to_owned()))?;
                        log::trace!("Wrote start event: {:?}", e);
                    }
                }
                b"v" => {
                    if in_t_cell {
                        in_t_cell_value = true;
                    } else {
                        writer.write_event(Event::Start(e.to_owned()))?;
                        log::trace!("Wrote start event: {:?}", e);
                    }
                }
                _ => {
                    writer.write_event(Event::Start(e.to_owned()))?;
                    log::trace!("Wrote start event: {:?}", e);
                }
            },
            Ok(Event::Text(e)) if in_t_cell_value => {
                let cell_value = e.unescape().unwrap();
                ss_index = Some(cell_value.parse::<usize>().unwrap());
                log::trace!("Shared string index: {:?}", ss_index);
                let si = &sst.si[ss_index.unwrap()];
                log::trace!("Shared string value: {:?}", si.t);
            }
            Ok(Event::Text(e)) => {
                writer.write_event(Event::Text(e.to_owned()))?;
                log::trace!("Wrote text event: {:?}", e);
            }
            Ok(Event::End(e)) => match e.name().as_ref() {
                b"v" => {
                    if in_t_cell_value {
                        in_t_cell_value = false;
                    } else {
                        writer.write_event(Event::End(e.to_owned()))?;
                        log::trace!("Wrote end event: {:?}", e);
                    }
                }
                b"c" => {
                    if in_t_cell {
                        let index = ss_index.unwrap();
                        let mut c_element = BytesStart::new("c");
                        c_element.push_attribute(("r", cell_ref.clone().unwrap().as_str()));
                        c_element.push_attribute(("t", "inlineStr"));
                        writer.write_event(Event::Start(c_element))?;
                        writer.write_event(Event::Start(BytesStart::new("is")))?;
                        writer.write_event(Event::Start(BytesStart::new("t")))?;
                        writer.write_event(Event::Text(BytesText::new(&sst.si[index].t)))?;
                        writer.write_event(Event::End(BytesEnd::new("t")))?;
                        writer.write_event(Event::End(BytesEnd::new("is")))?;
                        writer.write_event(Event::End(BytesEnd::new("c")))?;
                        log::trace!("Wrote inline string event");
                        ss_index = None;
                        cell_ref = None;
                        in_t_cell = false;
                    } else {
                        writer.write_event(Event::End(e.to_owned()))?;
                        log::trace!("Wrote end event: {:?}", e);
                    }
                }
                _ => {
                    writer.write_event(Event::End(e.to_owned()))?;
                    log::trace!("Wrote end event: {:?}", e);
                }
            },
            Ok(Event::Empty(e)) => {
                writer.write_event(Event::Empty(e.to_owned()))?;
                log::trace!("Wrote empty event: {:?}", e);
            }
            Ok(Event::Eof) => break,
            _ => {}
        }
        buf.clear();
    }
    buf.clear();

    let output_file = File::create(target_file_path)?;
    let mut file_writer = BufWriter::new(output_file);
    file_writer.write_all(&output)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;
    use test_log::test;

    mod test_schemas {
        pub mod worksheets {
            use crate::ooxml::schemas::shared_strings;
            use serde::{Deserialize, Serialize};

            #[derive(Serialize, Deserialize)]
            pub struct Worksheet {
                #[serde(rename = "@xmlns")]
                pub xmlns: String,
                #[serde(rename = "@xmlns:r")]
                pub xmlns_r: String,
                #[serde(rename = "@xmlns:mc")]
                pub xmlns_mc: String,
                #[serde(rename = "@Ignorable")]
                pub mc_ignorable: String,
                #[serde(rename = "@xmlns:x14ac")]
                pub xmlns_x14ac: String,
                #[serde(rename = "@xmlns:xr")]
                pub xmlns_xr: String,
                #[serde(rename = "@xmlns:xr2")]
                pub xmlns_xr2: String,
                #[serde(rename = "@xmlns:xr3")]
                pub xmlns_xr3: String,
                #[serde(rename = "@uid")]
                pub xr_uid: String,
                pub dimension: Dimension,
                #[serde(rename = "sheetViews")]
                pub sheet_views: SheetViews,
                #[serde(rename = "sheetFormatPr")]
                pub sheet_format_pr: SheetFormatPr,
                #[serde(rename = "sheetData")]
                pub sheet_data: SheetData,
                #[serde(rename = "pageMargins")]
                pub page_margins: PageMargins,
            }

            #[derive(Serialize, Deserialize)]
            pub struct Dimension {
                #[serde(rename = "@ref")]
                pub dimension_ref: String,
            }

            #[derive(Serialize, Deserialize)]
            pub struct SheetViews {
                #[serde(rename = "sheetView")]
                pub sheet_view: SheetView,
            }

            #[derive(Serialize, Deserialize)]
            pub struct SheetView {
                #[serde(rename = "@tabSelected")]
                pub tab_selected: String,
                #[serde(rename = "@workbookViewId")]
                pub workbook_view_id: String,
                #[serde(skip_serializing_if = "Option::is_none")]
                pub selection: Option<Selection>,
            }

            #[derive(Serialize, Deserialize)]
            pub struct Selection {
                #[serde(rename = "@activeCell")]
                pub active_cell: String,
                #[serde(rename = "@sqref")]
                pub sqref: String,
            }

            #[derive(Serialize, Deserialize)]
            pub struct SheetFormatPr {
                #[serde(rename = "@defaultRowHeight")]
                pub default_row_height: String,
                #[serde(rename = "@dyDescent")]
                pub x14ac_dy_descent: String,
            }

            #[derive(Serialize, Deserialize)]
            pub struct SheetData {
                pub row: Vec<Row>,
            }

            #[derive(Serialize, Deserialize)]
            pub struct Row {
                #[serde(rename = "@r")]
                pub r: String,
                #[serde(rename = "@spans")]
                pub spans: String,
                #[serde(rename = "@dyDescent")]
                pub x14ac_dy_descent: String,
                pub c: Vec<C>,
            }

            #[derive(Serialize, Deserialize)]
            pub struct C {
                #[serde(rename = "@r")]
                pub r: String,
                #[serde(rename = "@t", skip_serializing_if = "Option::is_none")]
                pub t: Option<String>,
                #[serde(rename = "@s", skip_serializing_if = "Option::is_none")]
                pub s: Option<String>,
                #[serde(skip_serializing_if = "Option::is_none")]
                pub v: Option<String>,
                #[serde(skip_serializing_if = "Option::is_none")]
                pub f: Option<String>,
                #[serde(skip_serializing_if = "Option::is_none")]
                pub is: Option<shared_strings::Si>,
            }

            #[derive(Serialize, Deserialize)]
            pub struct PageMargins {
                #[serde(rename = "@left")]
                pub left: String,
                #[serde(rename = "@right")]
                pub right: String,
                #[serde(rename = "@top")]
                pub top: String,
                #[serde(rename = "@bottom")]
                pub bottom: String,
                #[serde(rename = "@header")]
                pub header: String,
                #[serde(rename = "@footer")]
                pub footer: String,
            }
        }
    }

    #[test]
    fn test_read_shared_strings() {
        let sst: schemas::shared_strings::Sst =
            read_xml_file("tests/fixtures/simple_book.xlsx_ooxml/xl/sharedStrings.xml").unwrap();
        assert_eq!(sst.count, "2");
        assert_eq!(sst.unique_count, "2");
        assert_eq!(sst.si.len(), 2);
        assert_eq!(sst.si[0].t, "Hello");
        assert_eq!(sst.si[1].t, "World");
    }

    #[test]
    fn test_read_sheet() {
        let worksheet: test_schemas::worksheets::Worksheet =
            read_xml_file("tests/fixtures/simple_book.xlsx_ooxml/xl/worksheets/sheet1.xml")
                .unwrap();
        assert_eq!(worksheet.sheet_data.row.len(), 3);
        assert_eq!(worksheet.sheet_data.row[0].r, "1");
        assert_eq!(worksheet.sheet_data.row[0].spans, "1:1");
        assert_eq!(worksheet.sheet_data.row[0].c[0].t, Some("s".to_string()));
    }

    #[test]
    fn test_write_sheet() {
        let output_dir = tempdir().unwrap();
        let output_file = output_dir.path().join("sheet1.xml");
        let worksheet: test_schemas::worksheets::Worksheet =
            read_xml_file("tests/fixtures/simple_book.xlsx_ooxml/xl/worksheets/sheet1.xml")
                .unwrap();
        _write_xml_file(&output_file, &worksheet).unwrap();
        let output_file_path = output_file.to_str().unwrap();
        let new_worksheet: test_schemas::worksheets::Worksheet =
            read_xml_file(&output_file_path).unwrap();
        assert_eq!(new_worksheet.sheet_data.row[0].c[0].t.is_some(), true);
    }

    #[test]
    fn test_inline_strings() {
        let fixture =
            PathBuf::from("tests/fixtures/simple_book.xlsx_ooxml/xl/worksheets/sheet1.xml");
        let temp_dir = tempdir().unwrap();
        let output_file_path = temp_dir.path().join("sheet1.xml");
        fs::copy(&fixture, &output_file_path).unwrap();

        inline_strings(
            "tests/fixtures/simple_book.xlsx_ooxml/xl/sharedStrings.xml",
            output_file_path.to_str().unwrap(),
        )
        .unwrap();

        fs::copy(&output_file_path, PathBuf::from(".debug/out.xml")).unwrap();

        let new_worksheet: test_schemas::worksheets::Worksheet =
            read_xml_file(&output_file_path.to_str().unwrap()).unwrap();

        assert_eq!(
            new_worksheet.sheet_data.row[0].c[0].is.as_ref().unwrap().t,
            "Hello"
        );
    }
}
