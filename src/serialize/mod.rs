pub mod escape;
pub mod xml;

pub use escape::{escape_xml_str, escape_xml_write, needs_escaping, xml_safe_byte};
pub use xml::{
    bucket_symbols_by_rank, serialize_xml, serialize_xml_to_string, write_file_header,
    write_root_open, write_symbol_row, SerializeOptions, DEFAULT_STREAM_BUFFER_CAPACITY,
};

/// Trait for types that can be serialized as XML and converted to a string.
pub trait ToXmlString {
    fn write_xml<W: std::io::Write>(&self, out: &mut W) -> std::io::Result<()>;

    fn to_xml_string(&self) -> String {
        let mut buf = Vec::new();
        let _ = self.write_xml(&mut buf);
        String::from_utf8(buf).unwrap_or_default()
    }
}
