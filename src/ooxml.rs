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

#[inline(never)]
pub fn read_xml_file<T: DeserializeOwned>(file_path: &str) -> Result<T, io::Error> {
    let file = File::open(file_path)?;
    from_reader(BufReader::new(file)).map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
}

pub fn validate_xml_file(file_path: &str) -> Result<(), io::Error> {
    let file = File::open(file_path)?;
    let mut reader = Reader::from_reader(BufReader::new(file));
    reader.config_mut().trim_text(false);

    let mut buf = Vec::new();
    let mut depth = 0usize;
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Eof) => break,
            Ok(Event::Start(_)) => depth += 1,
            Ok(Event::End(_)) => depth = depth.saturating_sub(1),
            Ok(_) => {}
            Err(err) => return Err(io::Error::new(io::ErrorKind::InvalidData, err)),
        }
        buf.clear();
    }

    if depth != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "unexpected EOF: unclosed XML element(s)",
        ));
    }

    Ok(())
}

pub struct OoxmlBuffer {
    buffer: Vec<u8>,
    file_path: PathBuf,
}

impl OoxmlBuffer {
    fn local_name(name: &[u8]) -> &[u8] {
        name.rsplit(|b| *b == b':').next().unwrap_or(name)
    }

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

    fn remove_elements_by_local_name(mut self, local_names: &[&str]) -> Self {
        let mut reader = Reader::from_reader(&self.buffer[..]);
        let mut output = Vec::new();
        let mut writer = Writer::new(Cursor::new(&mut output));
        let local_names = local_names
            .iter()
            .map(|n| n.as_bytes().to_vec())
            .collect::<Vec<_>>();
        let mut skip_depth = 0usize;

        loop {
            match reader.read_event().unwrap() {
                Event::Eof => break,
                Event::Start(e) => {
                    if skip_depth > 0 {
                        skip_depth += 1;
                        continue;
                    }

                    let is_target = local_names
                        .iter()
                        .any(|n| Self::local_name(e.name().as_ref()) == n.as_slice());
                    if is_target {
                        skip_depth = 1;
                    } else {
                        writer.write_event(Event::Start(e.to_owned())).unwrap();
                    }
                }
                Event::End(e) => {
                    if skip_depth > 0 {
                        skip_depth -= 1;
                    } else {
                        writer.write_event(Event::End(e.to_owned())).unwrap();
                    }
                }
                Event::Empty(e) => {
                    if skip_depth > 0 {
                        continue;
                    }
                    let is_target = local_names
                        .iter()
                        .any(|n| Self::local_name(e.name().as_ref()) == n.as_slice());
                    if !is_target {
                        writer.write_event(Event::Empty(e.to_owned())).unwrap();
                    }
                }
                event => {
                    if skip_depth == 0 {
                        writer.write_event(event).unwrap();
                    }
                }
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
        let mut ss_index = None; // stores the parsed value of the current shared string index
        let mut ss_index_raw: Option<String> = None; // stores the raw value for lossless fallback

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
                                ss_index = None;
                                ss_index_raw = None;
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
                    _ => {
                        if !in_target_cell {
                            writer.write_event(Event::Start(e.to_owned())).unwrap();
                        }
                    }
                },
                // Match the text value within a target cell value block
                Event::Text(e) if in_target_cell_value => {
                    // Set the shared string index that we want to grab
                    let cell_value = e.decode().unwrap().into_owned();
                    ss_index_raw = Some(cell_value.clone());
                    ss_index = cell_value.trim().parse::<usize>().ok();
                }
                // Match some other text value and simply write it back
                Event::Text(e) => {
                    if !in_target_cell {
                        writer.write_event(Event::Text(e.to_owned())).unwrap();
                    }
                }
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
                            // Construct and write a new 'c' (cell) element
                            let mut c_element = BytesStart::new("c");

                            // Copy all original attributes back except the "t" attribute.
                            // We'll set it explicitly below based on whether we can inline.
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

                            if let Some(si) = ss_index.and_then(|idx| sst.si.get(idx)) {
                                // Replace the type with inlineStr
                                c_element.push_attribute(("t", "inlineStr"));
                                writer.write_event(Event::Start(c_element)).unwrap();

                                // Add the inner 'is' element (inline string)
                                writer
                                    .write_event(Event::Start(BytesStart::new("is")))
                                    .unwrap();

                                // Get the shared string value and write it in
                                if let Some(t) = &si.t {
                                    // Preserve xml:space where needed to keep lexical content compliant.
                                    let mut t_element = BytesStart::new("t");
                                    if let Some(xml_space) = &t.xml_space {
                                        t_element.push_attribute(("xml:space", xml_space.as_str()));
                                    }
                                    writer.write_event(Event::Start(t_element)).unwrap();
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
                            } else {
                                // Fallback to the original shared-string representation when index
                                // parsing or lookup fails so we don't emit a corrupt worksheet.
                                c_element.push_attribute(("t", "s"));
                                writer.write_event(Event::Start(c_element)).unwrap();
                                if let Some(raw) = ss_index_raw.as_ref() {
                                    writer
                                        .write_event(Event::Start(BytesStart::new("v")))
                                        .unwrap();
                                    writer
                                        .write_event(Event::Text(BytesText::new(raw)))
                                        .unwrap();
                                    writer.write_event(Event::End(BytesEnd::new("v"))).unwrap();
                                }
                                writer.write_event(Event::End(BytesEnd::new("c"))).unwrap();
                            }

                            // Reset the state
                            ss_index = None;
                            ss_index_raw = None;
                            cell_attrs = None;
                            in_target_cell = false;
                        } else {
                            // We're not in a target cell; just write and continue
                            writer.write_event(Event::End(e.to_owned())).unwrap();
                        }
                    }
                    _ => {
                        if !in_target_cell {
                            writer.write_event(Event::End(e.to_owned())).unwrap();
                        }
                    }
                },
                Event::Empty(e) => {
                    if !in_target_cell {
                        writer.write_event(Event::Empty(e.to_owned())).unwrap();
                    }
                }
                Event::Eof => break,
                event => {
                    if !in_target_cell {
                        writer.write_event(event).unwrap();
                    }
                }
            }
        }

        self.buffer = output;
        self
    }

    pub fn remove_calc_chain_relationship_entries(mut self) -> Self {
        let mut reader = Reader::from_reader(&self.buffer[..]);
        let mut output = Vec::new();
        let mut writer = Writer::new(Cursor::new(&mut output));

        loop {
            match reader.read_event().unwrap() {
                Event::Eof => break,
                Event::Empty(e) if e.name().as_ref() == b"Relationship" => {
                    let mut relationship_type: Option<String> = None;
                    let mut target: Option<String> = None;
                    for attr in e.attributes().with_checks(false).flatten() {
                        if attr.key.as_ref() == b"Type" {
                            relationship_type = Some(attr.unescape_value().unwrap().into_owned());
                        } else if attr.key.as_ref() == b"Target" {
                            target = Some(attr.unescape_value().unwrap().into_owned());
                        }
                    }

                    let is_calc_chain_type = relationship_type
                        .as_deref()
                        .map(|t| t.ends_with("/calcChain"))
                        .unwrap_or(false);
                    let is_calc_chain_target = target
                        .as_deref()
                        .map(|t| t.trim_start_matches('/').ends_with("calcChain.xml"))
                        .unwrap_or(false);

                    if !(is_calc_chain_type || is_calc_chain_target) {
                        writer.write_event(Event::Empty(e.to_owned())).unwrap();
                    }
                }
                event => writer.write_event(event).unwrap(),
            }
        }

        self.buffer = output;
        self
    }

    pub fn remove_calc_chain_content_type_override(mut self) -> Self {
        let mut reader = Reader::from_reader(&self.buffer[..]);
        let mut output = Vec::new();
        let mut writer = Writer::new(Cursor::new(&mut output));

        loop {
            match reader.read_event().unwrap() {
                Event::Eof => break,
                Event::Empty(e) if e.name().as_ref() == b"Override" => {
                    let mut part_name: Option<String> = None;
                    for attr in e.attributes().with_checks(false).flatten() {
                        if attr.key.as_ref() == b"PartName" {
                            part_name = Some(attr.unescape_value().unwrap().into_owned());
                        }
                    }

                    let is_calc_chain_override = part_name
                        .as_deref()
                        .map(|p| p.eq_ignore_ascii_case("/xl/calcChain.xml"))
                        .unwrap_or(false);

                    if !is_calc_chain_override {
                        writer.write_event(Event::Empty(e.to_owned())).unwrap();
                    }
                }
                event => writer.write_event(event).unwrap(),
            }
        }

        self.buffer = output;
        self
    }

    pub fn remove_volatile_core_properties(self) -> Self {
        self.remove_elements_by_local_name(&["lastModifiedBy", "revision", "modified"])
    }

    pub fn remove_volatile_app_properties(self) -> Self {
        self.remove_elements_by_local_name(&["TotalTime", "AppVersion"])
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

        // quick_xml's Reader splits text content at entity references (e.g.
        // `&amp;`, `&lt;`) into separate Text and GeneralRef events.  The
        // indenting Writer does not recognise GeneralRef as "text-like", so
        // consecutive GeneralRef events (such as `&lt;&gt;`) cause spurious
        // newlines and indentation to be inserted mid-content.  We work
        // around this by re-emitting each GeneralRef as a Text event
        // containing the original escaped entity (e.g. `&lt;`).  Entity
        // reference names are always ASCII, so the UTF-8 conversion is safe.
        loop {
            match reader.read_event().unwrap() {
                Event::Eof => break,
                Event::GeneralRef(e) => {
                    let ref_bytes = e.as_ref();
                    let mut entity = Vec::with_capacity(ref_bytes.len() + 2);
                    entity.push(b'&');
                    entity.extend_from_slice(ref_bytes);
                    entity.push(b';');
                    writer
                        .write_event(Event::Text(BytesText::from_escaped(
                            std::str::from_utf8(&entity).unwrap(),
                        )))
                        .unwrap();
                }
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

        assert_eq!(
            new_worksheet.sheet_data.row[1].c[3]
                .is
                .as_ref()
                .unwrap()
                .r
                .as_ref()
                .unwrap()[0]
                .t
                .xml_space
                .as_deref(),
            Some("preserve")
        );
    }

    #[test]
    fn test_remove_calc_chain_relationship_entries() {
        let fixture =
            PathBuf::from("tests/fixtures/simple_book.xlsx_ooxml/xl/_rels/workbook.xml.rels");
        let temp_dir = tempdir().unwrap();
        let output_file_path = temp_dir.path().join("workbook.xml.rels");
        fs::copy(&fixture, &output_file_path).unwrap();

        OoxmlBuffer::new(output_file_path.to_str().unwrap())
            .remove_calc_chain_relationship_entries()
            .save();

        let rels = fs::read_to_string(output_file_path).unwrap();
        assert!(!rels.contains("relationships/calcChain"));
        assert!(!rels.contains("Target=\"calcChain.xml\""));
    }

    #[test]
    fn test_remove_calc_chain_content_type_override() {
        let fixture = PathBuf::from("tests/fixtures/simple_book.xlsx_ooxml/[Content_Types].xml");
        let temp_dir = tempdir().unwrap();
        let output_file_path = temp_dir.path().join("[Content_Types].xml");
        fs::copy(&fixture, &output_file_path).unwrap();

        OoxmlBuffer::new(output_file_path.to_str().unwrap())
            .remove_calc_chain_content_type_override()
            .save();

        let content_types = fs::read_to_string(output_file_path).unwrap();
        assert!(!content_types.contains("/xl/calcChain.xml"));
    }

    #[test]
    fn test_read_xml_file_success_path() {
        #[derive(serde::Deserialize)]
        struct Root {
            value: String,
        }

        let temp_dir = tempdir().unwrap();
        let xml_path = temp_dir.path().join("root.xml");
        fs::write(&xml_path, "<root><value>ok</value></root>").unwrap();

        let parsed: Root = read_xml_file(xml_path.to_str().unwrap()).unwrap();
        assert_eq!(parsed.value, "ok");
    }

    #[test]
    fn test_read_xml_file_returns_invalid_data_for_malformed_xml() {
        #[derive(serde::Deserialize)]
        struct Root {
            #[serde(rename = "value")]
            _value: String,
        }

        let temp_dir = tempdir().unwrap();
        let xml_path = temp_dir.path().join("invalid.xml");
        fs::write(&xml_path, "<root><value>missing end").unwrap();

        let result: Result<Root, io::Error> = read_xml_file(xml_path.to_str().unwrap());
        let err = result.err().expect("malformed xml should return an error");
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn test_validate_xml_file_success_path() {
        let temp_dir = tempdir().unwrap();
        let xml_path = temp_dir.path().join("root.xml");
        fs::write(&xml_path, "<root><value>ok</value></root>").unwrap();

        let result = validate_xml_file(xml_path.to_str().unwrap());
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_xml_file_returns_invalid_data_for_malformed_xml() {
        let temp_dir = tempdir().unwrap();
        let xml_path = temp_dir.path().join("invalid.xml");
        fs::write(&xml_path, "<root><value>missing end").unwrap();

        let result = validate_xml_file(xml_path.to_str().unwrap());
        let err = result.err().expect("malformed xml should return an error");
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn test_tidy_removes_existing_declaration_before_save() {
        let temp_dir = tempdir().unwrap();
        let xml_path = temp_dir.path().join("decl.xml");
        fs::write(
            &xml_path,
            r#"<?xml version="1.0" encoding="UTF-8"?><root><a>1</a></root>"#,
        )
        .unwrap();

        OoxmlBuffer::new(xml_path.to_str().unwrap()).tidy().save();

        let xml = fs::read_to_string(xml_path).unwrap();
        assert_eq!(xml.matches("<?xml").count(), 1);
    }

    #[test]
    fn test_inline_shared_strings_preserves_non_shared_cells() {
        let temp_dir = tempdir().unwrap();
        let worksheet_path = temp_dir.path().join("sheet1.xml");
        fs::write(
            &worksheet_path,
            r#"<worksheet><sheetData><row r="1"><c r="A1" t="str"><v>42</v></c></row></sheetData></worksheet>"#,
        )
        .unwrap();

        let empty_sst = schemas::shared_strings::Sst {
            xmlns: String::from("http://schemas.openxmlformats.org/spreadsheetml/2006/main"),
            count: String::from("0"),
            unique_count: String::from("0"),
            si: vec![],
        };

        OoxmlBuffer::new(worksheet_path.to_str().unwrap())
            .inline_shared_strings(&empty_sst)
            .save();

        let worksheet = fs::read_to_string(worksheet_path).unwrap();
        assert!(worksheet.contains(r#"t="str""#));
        assert!(worksheet.contains("<v>42</v>"));
        assert!(!worksheet.contains(r#"t="inlineStr""#));
    }

    #[test]
    fn test_inline_shared_strings_falls_back_on_invalid_index() {
        let temp_dir = tempdir().unwrap();
        let worksheet_path = temp_dir.path().join("sheet1.xml");
        fs::write(
            &worksheet_path,
            r#"<worksheet><sheetData><row r="1"><c r="A1" t="s"><v>not-a-number</v></c></row></sheetData></worksheet>"#,
        )
        .unwrap();

        let sst: schemas::shared_strings::Sst =
            read_xml_file("tests/fixtures/simple_book.xlsx_ooxml/xl/sharedStrings.xml").unwrap();

        OoxmlBuffer::new(worksheet_path.to_str().unwrap())
            .inline_shared_strings(&sst)
            .save();

        let worksheet = fs::read_to_string(worksheet_path).unwrap();
        assert!(worksheet.contains(r#"t="s""#));
        assert!(worksheet.contains("<v>not-a-number</v>"));
        assert!(!worksheet.contains(r#"t="inlineStr""#));
    }

    #[test]
    fn test_remove_volatile_core_properties_with_namespaced_tags() {
        let temp_dir = tempdir().unwrap();
        let core_path = temp_dir.path().join("core.xml");
        fs::write(
            &core_path,
            r#"<cp:coreProperties xmlns:cp="x" xmlns:dcterms="y"><cp:lastModifiedBy>A</cp:lastModifiedBy><cp:revision>9</cp:revision><dcterms:modified>2026-01-01T00:00:00Z</dcterms:modified><dcterms:created>2020-01-01T00:00:00Z</dcterms:created></cp:coreProperties>"#,
        )
        .unwrap();

        OoxmlBuffer::new(core_path.to_str().unwrap())
            .remove_volatile_core_properties()
            .save();

        let xml = fs::read_to_string(core_path).unwrap();
        assert!(!xml.contains("lastModifiedBy"));
        assert!(!xml.contains("<cp:revision>"));
        assert!(!xml.contains("dcterms:modified"));
        assert!(xml.contains("dcterms:created"));
    }

    #[test]
    fn test_remove_volatile_app_properties() {
        let temp_dir = tempdir().unwrap();
        let app_path = temp_dir.path().join("app.xml");
        fs::write(
            &app_path,
            r#"<Properties xmlns="x"><Application>Word</Application><TotalTime>10</TotalTime><AppVersion>99</AppVersion></Properties>"#,
        )
        .unwrap();

        OoxmlBuffer::new(app_path.to_str().unwrap())
            .remove_volatile_app_properties()
            .save();

        let xml = fs::read_to_string(app_path).unwrap();
        assert!(xml.contains("<Application>Word</Application>"));
        assert!(!xml.contains("<TotalTime>"));
        assert!(!xml.contains("<AppVersion>"));
    }

    #[test]
    fn test_remove_volatile_app_properties_with_nested_and_empty_elements() {
        let temp_dir = tempdir().unwrap();
        let app_path = temp_dir.path().join("app_nested.xml");
        fs::write(
            &app_path,
            r#"<Properties xmlns="x"><Company/><TotalTime><nested><leaf/></nested></TotalTime><AppVersion/><Application>Excel</Application></Properties>"#,
        )
        .unwrap();

        OoxmlBuffer::new(app_path.to_str().unwrap())
            .remove_volatile_app_properties()
            .save();

        let xml = fs::read_to_string(app_path).unwrap();
        assert!(xml.contains("<Company/>"));
        assert!(xml.contains("<Application>Excel</Application>"));
        assert!(!xml.contains("<TotalTime>"));
        assert!(!xml.contains("<nested>"));
        assert!(!xml.contains("<leaf/>"));
        assert!(!xml.contains("<AppVersion/>"));
    }

    #[test]
    fn test_save_preserves_entity_references_in_text_content() {
        let temp_dir = tempdir().unwrap();
        let path = temp_dir.path().join("formula.xml");
        fs::write(
            &path,
            r#"<worksheet><sheetData><row r="1"><c r="A1" t="str" cm="1"><f t="array" ref="A1">_xlfn.LET(
  _xlpm.n, SUM(--(_xlpm.names&lt;&gt;"")),
  IF(_xlpm.n=0, "", _xlpm.conj &amp; " have")
)</f><v/></c></row></sheetData></worksheet>"#,
        )
        .unwrap();

        OoxmlBuffer::new(path.to_str().unwrap()).tidy().save();

        let xml = fs::read_to_string(&path).unwrap();
        // The &lt;&gt; entity pair must remain on the same line, not split
        // by indentation inserted by the pretty-printing writer.
        assert!(
            xml.contains("_xlpm.names&lt;&gt;\"\""),
            "entity references in text content were corrupted: {}",
            xml
        );
        assert!(
            xml.contains("_xlpm.conj &amp; \" have\""),
            "&amp; in text content was corrupted: {}",
            xml
        );
    }
}
