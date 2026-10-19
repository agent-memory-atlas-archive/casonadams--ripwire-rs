use super::harness::{ExpectedSym, GraphProtocolCase, verify_protocol_case};
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
            ExpectedSym { name: "MeshRenderer", kind: SymKind::Class, scope: None, has_body_span: true },
            ExpectedSym { name: "draw", kind: SymKind::Method, scope: Some("MeshRenderer"), has_body_span: true },
            ExpectedSym { name: "initPipeline", kind: SymKind::Function, scope: None, has_body_span: true },
        ],
        rejected_syms: &[],
        expected_edges: &[("draw", "initPipeline")],
        expect_external: false,
        expected_lego: Some(("BaseRenderer", "MeshRenderer")),
        expected_import: Some("Renderer.h"),
    });
    assert!(
        Lang::ObjC
            .semantics()
            .is_std_or_builtin_qualifier("NSString")
    );
    assert!(Lang::ObjC.semantics().is_std_or_builtin_member("alloc"));
}
