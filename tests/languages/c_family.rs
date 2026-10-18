use ripwire_rs::mcp::retrieval::call_lego;
use ripwire_rs::{Interner, Lang, LanguageRegistry, ingest_in_memory_files};

#[test]
fn test_cpp_capture_gates_filter_noise_and_keep_constants() {
    let registry = LanguageRegistry::new().unwrap();
    let interner = Interner::new();
    let cpp_src = br#"
static const int MAX_BUF = 4096;
constexpr int kValidConst = 7;
int mutable_global = 3;
int uninitialized_global;
__constant__ float d_const_table[16];
__device__ int D_DEV_CAP;
__device__ int d_mutable_dev;

struct Widget {
    int count = 0;
    static int s_mutable;
    static constexpr int kClassConst = 42;
};

void runner() {
    std::lock_guard<std::mutex> guard(mtx);
    if (guard.owns_lock()) {}
}

TEST_CASE("widget test") {
    runner();
}
"#;
    let c_src = b"static const int C_CONST = 1;\nint c_mut = 2;\n";
    let ingest = ingest_in_memory_files(
        &[("test.cpp", Lang::Cpp, cpp_src), ("test.c", Lang::C, c_src)],
        &registry,
        &interner,
    );
    let names: Vec<&str> = ingest.symbols.iter().map(|s| &interner[s.name]).collect();
    assert!(names.contains(&"MAX_BUF"));
    assert!(names.contains(&"kValidConst"));
    assert!(names.contains(&"kClassConst"));
    assert!(names.contains(&"count"));
    assert!(names.contains(&"runner"));
    assert!(names.contains(&"TEST_CASE"));
    assert!(names.contains(&"C_CONST"));
    assert!(!names.contains(&"mutable_global"));
    assert!(!names.contains(&"uninitialized_global"));
    assert!(!names.contains(&"s_mutable"));
    assert!(!names.contains(&"guard"));
    assert!(!names.contains(&"c_mut"));
}

#[test]
fn test_header_dialect_refinement_and_error_recovery_guard() {
    let registry = LanguageRegistry::new().unwrap();
    let interner = Interner::new();
    let cpp_hdr =
        b"namespace rw { inline bool kindIs(const char* a, const char* b) { return true; } }\n";
    let objc_hdr = b"@interface Renderer : NSObject\n- (void)draw;\n@end\n";
    let pure_c_hdr = b"int c_header_fn(int a);\n";
    let ingest = ingest_in_memory_files(
        &[
            ("include/nodekind.h", Lang::C, cpp_hdr),
            ("include/Renderer.h", Lang::Cpp, objc_hdr),
            ("include/pure.h", Lang::C, pure_c_hdr),
        ],
        &registry,
        &interner,
    );
    assert_eq!(ingest.files[0].lang, Lang::Cpp);
    assert_eq!(ingest.files[1].lang, Lang::ObjC);
    assert_eq!(ingest.files[2].lang, Lang::C);
    let names: Vec<&str> = ingest.symbols.iter().map(|s| &interner[s.name]).collect();
    assert!(names.contains(&"kindIs"));
}

#[test]
fn test_macro_blanking_reparse_and_macro_body_calls() {
    let registry = LanguageRegistry::new().unwrap();
    let interner = Interner::new();
    let src = br#"
#define RW_CHECK_(x) handleAssert(x)
#define ASSUME(x) RW_CHECK_(x)

struct LanternRecord {
    int id = 0;
    TAG_KIND(LanternRecord)
};

struct MooringRecord {
    int id = 0;
    TAG_KIND(MooringRecord)
};

int countMoorings(int a) {
    ASSUME(a > 0);
    return a;
}
"#;
    let ingest = ingest_in_memory_files(&[("src/leak.cpp", Lang::Cpp, src)], &registry, &interner);
    let names: Vec<&str> = ingest.symbols.iter().map(|s| &interner[s.name]).collect();
    assert!(names.contains(&"countMoorings"));
    assert!(names.contains(&"ASSUME"));
    assert!(names.contains(&"RW_CHECK_"));
    let assume_sym = ingest
        .symbols
        .iter()
        .find(|s| &interner[s.name] == "ASSUME")
        .unwrap();
    let has_macro_edge = ingest
        .references
        .iter()
        .any(|r| r.from_symbol == assume_sym.id && &interner[r.callee_name] == "RW_CHECK_");
    assert!(has_macro_edge);
}

#[test]
fn test_cpp_attribute_macro_decorated_methods_extracted() {
    let registry = LanguageRegistry::new().unwrap();
    let interner = Interner::new();
    let hdr = br#"
class ConsoleLog {
public:
    RW_COLD_NOINLINE
    static void handleAssert(int kind, const char* expr) noexcept DIAGNOSTICS_ANALYZER_NORETURN;
    [[noreturn]] RW_COLD_NOINLINE
    static void handlePanic(const char* file, int line) noexcept;
};
"#;
    let cpp = br#"
RW_COLD_NOINLINE
void ConsoleLog::handleAssert(int kind, const char* expr) noexcept {
}
"#;
    let ingest = ingest_in_memory_files(
        &[
            ("src/infra/Diagnostics.h", Lang::Cpp, hdr),
            ("src/infra/diagnostics.cpp", Lang::Cpp, cpp),
        ],
        &registry,
        &interner,
    );
    let lego = call_lego(&ingest, &interner, "ConsoleLog");
    let methods = lego["methods"].as_array().unwrap();
    assert_eq!(methods.len(), 2);
}
