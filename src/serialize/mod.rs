pub mod escape;
pub mod html;
pub mod query_xml;
pub mod writer;
pub mod xml;

pub use writer::{MinifyingWriter, XmlWriter};

pub use query_xml::{
    CommunityModule, CommunitySummary, TreeFileSymbols, serialize_around_xml,
    serialize_communities_with_summary_xml, serialize_communities_xml, serialize_connect_xml,
    serialize_dead_code_xml, serialize_deps_xml, serialize_doc_drift_xml, serialize_exemplar_xml,
    serialize_exercises_xml, serialize_expand_xml, serialize_external_surface_xml,
    serialize_flags_xml, serialize_for_xml, serialize_from_trace_xml, serialize_grep_xml,
    serialize_lego_xml, serialize_mentions_xml, serialize_owners_xml, serialize_pack_top_n_xml,
    serialize_path_between_xml, serialize_recall_xml, serialize_safe_delete_xml,
    serialize_stray_content_xml, serialize_test_gate_xml, serialize_tree_xml,
    serialize_whereis_xml, serialize_zoom_xml,
};

pub use html::{ColorBy, generate_call_graph_html, write_call_graph_html};

pub use escape::{escape_xml_str, escape_xml_write, needs_escaping, xml_safe_byte};
pub use xml::{
    CountingWriter, DEFAULT_PACK_BUDGET_BYTES, DEFAULT_STREAM_BUFFER_CAPACITY, RootElementContext,
    SerializeOptions, StreamXmlContext, bucket_symbols_by_rank, serialize_xml,
    serialize_xml_to_string, serialize_xml_with_ranks, write_file_header, write_root_open,
    write_root_open_with_rank_by, write_symbol_row,
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
