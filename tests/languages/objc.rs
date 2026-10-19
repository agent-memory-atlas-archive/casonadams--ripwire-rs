use super::harness::{GraphProtocolCase, sym, verify_protocol_case};
use ripwire_rs::{Lang, SymKind};

#[test]
fn test_objc_graph_protocol() {
    verify_protocol_case(&GraphProtocolCase {
        label: "objc",
        files: &[(
            "src/Renderer.m",
            Lang::ObjC,
            b"#import \"Renderer.h\"\n@interface BaseRenderer\n@end\n@interface MeshRenderer : BaseRenderer\n@end\n@implementation MeshRenderer\n- (void)draw {\n    NSString *s;\n    initPipeline();\n}\n@end\nvoid initPipeline(void) {\n    return;\n}\n",
        )],
        expected_syms: &[
            sym("MeshRenderer", SymKind::Class, None, true),
            sym("draw", SymKind::Method, Some("MeshRenderer"), true),
            sym("initPipeline", SymKind::Function, None, true),
        ],
        expected_edges: &[("draw", "initPipeline")],
        expected_lego: Some(("BaseRenderer", "MeshRenderer")),
        expected_import: Some("Renderer.h"),
        ..Default::default()
    });
    assert!(
        Lang::ObjC
            .semantics()
            .is_std_or_builtin_qualifier("NSString")
    );
    assert!(Lang::ObjC.semantics().is_std_or_builtin_member("alloc"));
}
