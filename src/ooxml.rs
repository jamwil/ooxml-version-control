use quick_xml::de::from_reader;
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use quick_xml::se::to_utf8_io_writer;
use quick_xml::{Reader, Writer};
use serde::de::DeserializeOwned;
use std::fs::File;
use std::io::{self, BufReader, Cursor};
use std::path::PathBuf;

pub mod schemas {
    pub mod shared_strings {
        use serde::{Deserialize, Serialize};

        #[derive(Serialize, Deserialize, Debug, Clone)]
        pub struct Text {
            #[serde(rename = "$text")]
            pub text: String,
            #[serde(rename = "@xml:space")]
            pub xml_space: Option<String>,
        }

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
            #[serde(skip_serializing_if = "Option::is_none")]
            pub t: Option<Text>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub r: Option<Vec<R>>,
        }

        #[derive(Serialize, Deserialize, Debug, Clone)]
        #[serde(rename = "r")]
        pub struct R {
            #[serde(rename = "rPr")]
            pub r_pr: Option<Rpr>,
            pub t: Text,
        }

        #[derive(Serialize, Deserialize, Debug, Clone)]
        pub struct Rpr {
            #[serde(rename = "rFont", skip_serializing_if = "Option::is_none")]
            pub r_font: Option<Val>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub charset: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub family: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub b: Option<BooleanProperty>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub i: Option<BooleanProperty>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub strike: Option<Val>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub outline: Option<Val>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub shadow: Option<Val>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub condense: Option<Val>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub extend: Option<Val>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub color: Option<Color>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub sz: Option<Val>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub u: Option<Val>,
            #[serde(rename = "vertAlign", skip_serializing_if = "Option::is_none")]
            pub vert_align: Option<Val>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub scheme: Option<Val>,
        }

        #[derive(Serialize, Deserialize, Debug, Clone)]
        pub struct BooleanProperty {
            #[serde(rename = "@val", default, skip_serializing_if = "Option::is_none")]
            pub val: Option<String>,
        }

        #[derive(Serialize, Deserialize, Debug, Clone)]
        pub struct Val {
            #[serde(rename = "@val")]
            pub val: String,
        }

        #[derive(Serialize, Deserialize, Debug, Clone)]
        pub struct Color {
            #[serde(rename = "@auto", skip_serializing_if = "Option::is_none")]
            pub auto: Option<String>,

            #[serde(rename = "@indexed", skip_serializing_if = "Option::is_none")]
            pub indexed: Option<String>,

            #[serde(rename = "@rgb", skip_serializing_if = "Option::is_none")]
            pub rgb: Option<String>,

            #[serde(rename = "@theme", skip_serializing_if = "Option::is_none")]
            pub theme: Option<String>,

            #[serde(rename = "@tint", skip_serializing_if = "Option::is_none")]
            pub tint: Option<String>,
        }
    }
}

pub fn read_xml_file<T: DeserializeOwned>(file_path: &str) -> Result<T, io::Error> {
    let file = File::open(file_path)?;
    let reader = BufReader::new(file);
    let result: T = from_reader(reader).unwrap();

    Ok(result)
}

pub struct OoxmlBuffer {
    buffer: Vec<u8>,
    file_path: PathBuf,
}

impl OoxmlBuffer {
    pub fn new(file_path: &str) -> Self {
        let mut output = Vec::new();
        let mut writer = Writer::new(Cursor::new(&mut output));

        let mut reader = Reader::from_file(file_path).unwrap();
        reader.config_mut().trim_text(false);

        let mut buf = Vec::new();
        loop {
            match reader.read_event_into(&mut buf).unwrap() {
                Event::Eof => break,
                event => writer.write_event(event).unwrap(),
            }
            buf.clear();
        }

        Self {
            buffer: output,
            file_path: PathBuf::from(file_path),
        }
    }

    pub fn tidy(mut self) -> Self {
        let mut reader = Reader::from_reader(&self.buffer[..]);
        let mut output = Vec::new();
        let mut writer = Writer::new(Cursor::new(&mut output));

        loop {
            match reader.read_event().unwrap() {
                Event::Eof => break,
                Event::Comment(_) | Event::Decl(_) | Event::PI(_) | Event::DocType(_) => {}
                event => writer.write_event(event).unwrap(),
            }
        }

        self.buffer = output;
        self
    }

    pub fn inline_shared_strings(mut self, sst: &schemas::shared_strings::Sst) -> Self {
        let mut reader = Reader::from_reader(&self.buffer[..]);
        let mut output = Vec::new();
        let mut buffer = Cursor::new(&mut output);
        let mut writer = Writer::new(&mut buffer);

        // As we loop through the events in the xml hierarchy, we'll try to catch
        // cells that reference a shared string. We'll use some function-level state
        // to set flags that tell us we're in a block that should be intercepted
        // and values which we'll need to write in.
        //
        // A shared string cell takes the form <c> <v> index </v> </c>
        // The index is the index from the Si field in the Sst schema

        let mut in_target_cell = false; // set to true when cell type is 's', i.e., shared string
        let mut in_target_cell_value = false;
        let mut cell_attrs: Option<Vec<(Vec<u8>, String)>> = None; // stores original attributes as owned key/value pairs
        let mut ss_index = None; // stores the value of the current shared string index

        loop {
            // Begin looping through xml events and intercepting shared string cells
            match reader.read_event().unwrap() {
                Event::Start(e) => match e.name().as_ref() {
                    // Match the start of a cell block
                    b"c" => {
                        // Collect the cell attributes into an owned Vec of (key, value) pairs
                        let raw_attributes = e.attributes().collect::<Result<Vec<_>, _>>().unwrap();
                        let attrs_pairs = raw_attributes
                            .into_iter()
                            .map(|a| {
                                (
                                    a.key.as_ref().to_vec(),
                                    a.unescape_value().unwrap().into_owned(),
                                )
                            })
                            .collect::<Vec<(Vec<u8>, String)>>();

                        // Look for a 't' (type) attribute
                        if let Some((_, cell_type)) =
                            attrs_pairs.iter().find(|(k, _)| k.as_slice() == b"t")
                        {
                            // Shared-string cells have type 's'
                            if cell_type.as_str() == "s" {
                                in_target_cell = true;

                                // Preserve attributes for later reuse
                                cell_attrs = Some(attrs_pairs);
                            } else {
                                // Non shared-string cell – write unchanged
                                writer.write_event(Event::Start(e.to_owned())).unwrap();
                            }
                        } else {
                            // No type attribute – write unchanged
                            writer.write_event(Event::Start(e.to_owned())).unwrap();
                        }
                    }
                    // Match the start of a value block
                    b"v" => {
                        // Is our v block within a c block?
                        if in_target_cell {
                            // Let's set the flag
                            in_target_cell_value = true;
                        } else {
                            // Nothing to see here
                            writer.write_event(Event::Start(e.to_owned())).unwrap();
                        }
                    }
                    _ => writer.write_event(Event::Start(e.to_owned())).unwrap(),
                },
                // Match the text value within a target cell value block
                Event::Text(e) if in_target_cell_value => {
                    // Set the shared string index that we want to grab
                    let cell_value = e.decode().unwrap();
                    ss_index = Some(cell_value.parse::<usize>().unwrap());
                }
                // Match some other text value and simply write it back
                Event::Text(e) => writer.write_event(Event::Text(e.to_owned())).unwrap(),
                // Match the end of a block and rewrite the block if we're in a target cell
                Event::End(e) => match e.name().as_ref() {
                    // We're at a closing value tag
                    b"v" => {
                        if in_target_cell_value {
                            // Unset the flag
                            in_target_cell_value = false;
                        } else {
                            // Just write the event like normal
                            writer.write_event(Event::End(e.to_owned())).unwrap();
                        }
                    }
                    // We're at a closing cell tag; this is where we'll rewrite the cell
                    // in order to inline the string
                    b"c" => {
                        // Are we in a target cell or is this some other cell that we don't
                        // want to modify?
                        if in_target_cell {
                            // We're in a target cell -- let's inline the string
                            let index = ss_index.expect("Missing shared string index");

                            // We'll construct a new 'c' element that takes the form:
                            // <c> <is> <t> raw string value </t> </is> </c>

                            // Construct and write a new 'c' (cell) element
                            let mut c_element = BytesStart::new("c");

                            // Copy all original attributes back except the "t" attribute
                            if let Some(attrs) = cell_attrs.as_ref() {
                                for (k, v) in attrs {
                                    if k.as_slice() == b"t" {
                                        continue;
                                    }
                                    let key = std::str::from_utf8(k)
                                        .expect("Invalid UTF-8 in attribute name");
                                    c_element.push_attribute((key, v.as_str()));
                                }
                            }

                            // Replace the type with inlineStr
                            c_element.push_attribute(("t", "inlineStr"));
                            writer.write_event(Event::Start(c_element)).unwrap();

                            // Add the inner 'is' element (inline string)
                            writer
                                .write_event(Event::Start(BytesStart::new("is")))
                                .unwrap();

                            // Get the shared string value and write it in
                            let si = &sst.si[index];
                            if let Some(t) = &si.t {
                                // The shared string is a simple text value
                                writer
                                    .write_event(Event::Start(BytesStart::new("t")))
                                    .unwrap();
                                writer
                                    .write_event(Event::Text(BytesText::new(&t.text)))
                                    .unwrap();
                                writer.write_event(Event::End(BytesEnd::new("t"))).unwrap();
                            } else if let Some(r) = &si.r {
                                // The shared string has inline formatting
                                for item in r {
                                    to_utf8_io_writer(&mut writer.get_mut(), &item).unwrap();
                                }
                            }

                            writer.write_event(Event::End(BytesEnd::new("is"))).unwrap();
                            writer.write_event(Event::End(BytesEnd::new("c"))).unwrap();

                            // Reset the state
                            ss_index = None;
                            cell_attrs = None;
                            in_target_cell = false;
                        } else {
                            // We're not in a target cell; just write and continue
                            writer.write_event(Event::End(e.to_owned())).unwrap();
                        }
                    }
                    _ => writer.write_event(Event::End(e.to_owned())).unwrap(),
                },
                Event::Empty(e) => writer.write_event(Event::Empty(e.to_owned())).unwrap(),
                Event::Eof => break,
                event => writer.write_event(event).unwrap(),
            }
        }

        self.buffer = output;
        self
    }

    pub fn save(self) {
        let mut reader = Reader::from_reader(&self.buffer[..]);
        let mut output = Vec::new();
        let mut writer = Writer::new_with_indent(Cursor::new(&mut output), b' ', 4);

        writer
            .write_event(Event::Decl(BytesDecl::new(
                "1.0",
                Some("UTF-8"),
                Some("yes"),
            )))
            .unwrap();

        loop {
            match reader.read_event().unwrap() {
                Event::Eof => break,
                event => writer.write_event(event).unwrap(),
            }
        }

        std::fs::write(&self.file_path, output).unwrap();
    }
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
                pub xmlns: Option<String>,
                #[serde(rename = "@xmlns:r")]
                pub xmlns_r: Option<String>,
                #[serde(rename = "@xmlns:mc")]
                pub xmlns_mc: Option<String>,
                #[serde(rename = "@Ignorable")]
                pub mc_ignorable: Option<String>,
                #[serde(rename = "@xmlns:x14ac")]
                pub xmlns_x14ac: Option<String>,
                #[serde(rename = "@xmlns:xr")]
                pub xmlns_xr: Option<String>,
                #[serde(rename = "@xmlns:xr2")]
                pub xmlns_xr2: Option<String>,
                #[serde(rename = "@xmlns:xr3")]
                pub xmlns_xr3: Option<String>,
                #[serde(rename = "@uid")]
                pub xr_uid: Option<String>,
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
                pub dimension_ref: Option<String>,
            }

            #[derive(Serialize, Deserialize)]
            pub struct SheetViews {
                #[serde(rename = "sheetView")]
                pub sheet_view: SheetView,
            }

            #[derive(Serialize, Deserialize)]
            pub struct SheetView {
                #[serde(rename = "@tabSelected")]
                pub tab_selected: Option<String>,
                #[serde(rename = "@workbookViewId")]
                pub workbook_view_id: Option<String>,
                #[serde(skip_serializing_if = "Option::is_none")]
                pub selection: Option<Selection>,
            }

            #[derive(Serialize, Deserialize)]
            pub struct Selection {
                #[serde(rename = "@activeCell")]
                pub active_cell: Option<String>,
                #[serde(rename = "@sqref")]
                pub sqref: Option<String>,
            }

            #[derive(Serialize, Deserialize)]
            pub struct SheetFormatPr {
                #[serde(rename = "@defaultRowHeight")]
                pub default_row_height: Option<String>,
                #[serde(rename = "@dyDescent")]
                pub x14ac_dy_descent: Option<String>,
            }

            #[derive(Serialize, Deserialize)]
            pub struct SheetData {
                pub row: Vec<Row>,
            }

            #[derive(Serialize, Deserialize)]
            pub struct Row {
                #[serde(rename = "@r")]
                pub r: Option<String>,
                #[serde(rename = "@spans")]
                pub spans: Option<String>,
                #[serde(rename = "@dyDescent")]
                pub x14ac_dy_descent: Option<String>,
                pub c: Vec<C>,
            }

            #[derive(Serialize, Deserialize)]
            pub struct C {
                #[serde(rename = "@r")]
                pub r: Option<String>,
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
                pub left: Option<String>,
                #[serde(rename = "@right")]
                pub right: Option<String>,
                #[serde(rename = "@top")]
                pub top: Option<String>,
                #[serde(rename = "@bottom")]
                pub bottom: Option<String>,
                #[serde(rename = "@header")]
                pub header: Option<String>,
                #[serde(rename = "@footer")]
                pub footer: Option<String>,
            }
        }
    }

    #[test]
    fn test_read_shared_strings_without_inline_formatting() {
        let sst: schemas::shared_strings::Sst =
            read_xml_file("tests/fixtures/simple_book.xlsx_ooxml/xl/sharedStrings.xml").unwrap();
        assert_eq!(sst.count, "2");
        assert_eq!(sst.unique_count, "2");
        assert_eq!(sst.si.len(), 2);
        assert_eq!(sst.si[0].t.as_ref().unwrap().text, "Hello");
        assert_eq!(sst.si[1].t.as_ref().unwrap().text, "World");
    }

    #[test]
    fn test_read_shared_strings_with_inline_formatting() {
        let sst: schemas::shared_strings::Sst =
            read_xml_file("tests/fixtures/complex_book.xlsx_ooxml/xl/sharedStrings.xml").unwrap();
        assert_eq!(sst.count, "7");
        assert_eq!(sst.unique_count, "7");
        assert_eq!(sst.si.len(), 7);
        assert_eq!(sst.si[6].r.as_ref().unwrap()[0].t.text, "fun ");
    }

    #[test]
    fn test_read_shared_strings_with_non_existant_file() {
        let default_sst = schemas::shared_strings::Sst {
            xmlns: String::from("http://schemas.openxmlformats.org/spreadsheetml/2006/main"),
            count: String::from("0"),
            unique_count: String::from("0"),
            si: vec![],
        };
        let sst: schemas::shared_strings::Sst =
            read_xml_file("tests/fixtures/simple_book.xlsx_ooxml/oops.xml").unwrap_or(default_sst);
        assert_eq!(sst.count, "0");
        assert_eq!(sst.unique_count, "0");
        assert_eq!(sst.si.len(), 0);
    }

    #[test]
    fn test_read_sheet() {
        let worksheet: test_schemas::worksheets::Worksheet =
            read_xml_file("tests/fixtures/simple_book.xlsx_ooxml/xl/worksheets/sheet1.xml")
                .unwrap();
        assert_eq!(worksheet.sheet_data.row.len(), 3);
        assert_eq!(worksheet.sheet_data.row[0].r.as_ref().unwrap(), "1");
        assert_eq!(worksheet.sheet_data.row[0].spans.as_ref().unwrap(), "1:1");
        assert_eq!(worksheet.sheet_data.row[0].c[0].t, Some("s".to_string()));
    }

    #[test]
    fn test_inline_strings_without_inline_formatting() {
        let fixture =
            PathBuf::from("tests/fixtures/simple_book.xlsx_ooxml/xl/worksheets/sheet1.xml");
        let temp_dir = tempdir().unwrap();
        let output_file_path = temp_dir.path().join("sheet1.xml");
        fs::copy(&fixture, &output_file_path).unwrap();

        let sst: schemas::shared_strings::Sst =
            read_xml_file("tests/fixtures/simple_book.xlsx_ooxml/xl/sharedStrings.xml").unwrap();

        OoxmlBuffer::new(output_file_path.to_str().unwrap())
            .tidy()
            .inline_shared_strings(&sst)
            .save();

        let new_worksheet: test_schemas::worksheets::Worksheet =
            read_xml_file(output_file_path.to_str().unwrap()).unwrap();

        assert_eq!(
            new_worksheet.sheet_data.row[0].c[0]
                .is
                .as_ref()
                .unwrap()
                .t
                .as_ref()
                .unwrap()
                .text,
            "Hello"
        );
    }

    #[test]
    fn test_inline_strings_with_inline_formatting() {
        let fixture =
            PathBuf::from("tests/fixtures/complex_book.xlsx_ooxml/xl/worksheets/sheet1.xml");
        let temp_dir = tempdir().unwrap();
        let output_file_path = temp_dir.path().join("sheet1.xml");
        fs::copy(&fixture, &output_file_path).unwrap();

        let sst: schemas::shared_strings::Sst =
            read_xml_file("tests/fixtures/complex_book.xlsx_ooxml/xl/sharedStrings.xml").unwrap();

        OoxmlBuffer::new(output_file_path.to_str().unwrap())
            .tidy()
            .inline_shared_strings(&sst)
            .save();

        let new_worksheet: test_schemas::worksheets::Worksheet =
            read_xml_file(output_file_path.to_str().unwrap()).unwrap();

        assert_eq!(
            new_worksheet.sheet_data.row[1].c[3]
                .is
                .as_ref()
                .unwrap()
                .r
                .as_ref()
                .unwrap()[0]
                .t
                .text,
            "fun "
        );

        assert_eq!(
            new_worksheet.sheet_data.row[1].c[3]
                .is
                .as_ref()
                .unwrap()
                .r
                .as_ref()
                .unwrap()[1]
                .t
                .text,
            "and"
        );

        assert_eq!(
            new_worksheet.sheet_data.row[1].c[3]
                .is
                .as_ref()
                .unwrap()
                .r
                .as_ref()
                .unwrap()[2]
                .t
                .text,
            " "
        );
    }
}
