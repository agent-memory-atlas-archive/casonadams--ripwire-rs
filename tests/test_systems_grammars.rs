use rwire::{ingest_in_memory_files, Graph, IngestResult, Interner, Lang, LanguageRegistry};

#[test]
fn test_systems_grammars() {
    // ── 1. C++ FIXTURE: templates, classes, methods, macros, calls ────────────
    let cpp_code = r#"
#define MAX_BUFFER 4096

template <typename T>
class Container {
public:
    void add(T item) {
        process(item);
    }
private:
    void process(T item) {}
};

int add_numbers(int a, int b) {
    return a + b;
}

int main() {
    return add_numbers(2, 3);
}
"#;

    // ── 2. CUDA FIXTURE: kernel launch <<<...>>> and kernel definitions ───────
    let cuda_code = r#"
__global__ void vectorAdd(const float* a, const float* b, float* c, int n) {
    int i = blockDim.x * blockIdx.x + threadIdx.x;
    if (i < n) {
        c[i] = a[i] + b[i];
    }
}

void launch_kernel(float* d_a, float* d_b, float* d_c, int n) {
    vectorAdd<<<1, 256>>>(d_a, d_b, d_c, n);
}
"#;

    // ── 3. JAVA FIXTURE: interfaces, classes, hierarchy, methods ─────────────
    let java_code = r#"
package com.example;

public interface Service {
    void execute();
}

public class Worker implements Service {
    public void execute() {
        processTask();
    }
    private void processTask() {}
}
"#;

    // ── 4. C# FIXTURE: namespaces, interfaces, methods, classes ──────────────
    let csharp_code = r#"
namespace Enterprise.System {
    public interface IRepository {
        void Save();
    }

    public class SqlRepository : IRepository {
        public void Save() {
            ConnectDb();
        }
        private void ConnectDb() {}
    }
}
"#;

    // ── 5. SWIFT FIXTURE: protocols, structs, methods, calls ──────────────────
    let swift_code = r#"
protocol Greeter {
    func greet()
}

struct ConsoleGreeter: Greeter {
    func greet() {
        logMessage()
    }
    func logMessage() {}
}
"#;

    // ── 6. BASH FIXTURE: function definitions and invocations ────────────────
    let bash_code = r#"
#!/usr/bin/env bash

init_environment() {
    setup_paths
}

setup_paths() {
    export PATH="/custom/bin:$PATH"
}

init_environment
"#;

    // In-memory ingestion: 100% zero disk I/O, zero network
    let files: &[(&str, Lang, &[u8])] = &[
        ("engine.cpp", Lang::Cpp, cpp_code.as_bytes()),
        ("kernel.cu", Lang::Cuda, cuda_code.as_bytes()),
        ("Worker.java", Lang::Java, java_code.as_bytes()),
        ("Repository.cs", Lang::CSharp, csharp_code.as_bytes()),
        ("Greeter.swift", Lang::Swift, swift_code.as_bytes()),
        ("deploy.sh", Lang::Bash, bash_code.as_bytes()),
    ];

    let registry = LanguageRegistry::new().expect("failed to load all supported grammars");
    let interner = Interner::new();
    let ingest: IngestResult = ingest_in_memory_files(files, &registry, &interner);

    assert_eq!(ingest.file_count(), 6);
    assert!(
        ingest.symbol_count() >= 15,
        "Expected at least 15 symbols across systems languages, found {}",
        ingest.symbol_count()
    );

    let sym_names: Vec<String> = ingest
        .symbols
        .iter()
        .map(|s| interner.resolve(&s.name).to_string())
        .collect();

    // Assert C++ symbols
    assert!(sym_names.contains(&"Container".to_string()));
    assert!(sym_names.contains(&"add".to_string()));
    assert!(sym_names.contains(&"add_numbers".to_string()));
    assert!(sym_names.contains(&"main".to_string()));

    // Assert CUDA symbols
    assert!(sym_names.contains(&"vectorAdd".to_string()));
    assert!(sym_names.contains(&"launch_kernel".to_string()));

    // Assert Java symbols
    assert!(sym_names.contains(&"Service".to_string()));
    assert!(sym_names.contains(&"Worker".to_string()));

    // Assert C# symbols
    assert!(sym_names.contains(&"IRepository".to_string()));
    assert!(sym_names.contains(&"SqlRepository".to_string()));

    // Assert Swift symbols
    assert!(sym_names.contains(&"ConsoleGreeter".to_string()));

    // Assert Bash symbols
    assert!(sym_names.contains(&"init_environment".to_string()));
    assert!(sym_names.contains(&"setup_paths".to_string()));

    // Verify Graph assembly and Call Resolution across these languages
    let graph = Graph::build(&ingest, &interner);
    assert!(
        graph.edge_count() >= 5,
        "Expected at least 5 resolved call edges, found {}",
        graph.edge_count()
    );
}
