pub mod escape;
pub mod xml;

pub use escape::{escape_xml_str, escape_xml_write, needs_escaping, xml_safe_byte};
pub use xml::{
    bucket_symbols_by_rank, serialize_xml, serialize_xml_to_string, write_file_header,
    write_root_open, write_symbol_row, SerializeOptions, DEFAULT_STREAM_BUFFER_CAPACITY,
};
