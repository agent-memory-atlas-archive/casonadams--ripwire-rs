use ripwire_rs::{Graph, IngestResult, Interner, Lang, LanguageRegistry, ingest_in_memory_files};

#[test]
fn test_systems_grammars() {
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

    assert!(sym_names.contains(&"Container".to_string()));
    assert!(sym_names.contains(&"add".to_string()));
    assert!(sym_names.contains(&"add_numbers".to_string()));
    assert!(sym_names.contains(&"main".to_string()));
    assert!(sym_names.contains(&"vectorAdd".to_string()));
    assert!(sym_names.contains(&"launch_kernel".to_string()));
    assert!(sym_names.contains(&"Service".to_string()));
    assert!(sym_names.contains(&"Worker".to_string()));
    assert!(sym_names.contains(&"IRepository".to_string()));
    assert!(sym_names.contains(&"SqlRepository".to_string()));
    assert!(sym_names.contains(&"ConsoleGreeter".to_string()));
    assert!(sym_names.contains(&"init_environment".to_string()));
    assert!(sym_names.contains(&"setup_paths".to_string()));

    let graph = Graph::build(&ingest, &interner);
    assert!(
        graph.edge_count() >= 5,
        "Expected at least 5 resolved call edges, found {}",
        graph.edge_count()
    );
}

#[test]
fn test_tier1_cloud_and_systems_grammars() {
    let zig_code = r#"
pub fn add(a: i32, b: i32) i32 {
    return a + b;
}

pub fn calculate() i32 {
    return add(10, 20);
}
"#;

    let solidity_code = r#"
contract Token {
    function transfer(address to, uint amount) public {
        emitTransfer(to, amount);
    }
}
"#;

    let hcl_code = r#"
resource "aws_s3_bucket" "b" {
  bucket = "my-tf-test-bucket"
}
"#;

    let scala_code = r#"
object MainApp {
  def processOrder(id: Int): Unit = {
    println(id)
  }
}
"#;

    let erlang_code = r#"
-module(server).
-export([start/0]).

start() ->
    ok.
"#;

    let html_code = r#"
<!DOCTYPE html>
<html>
  <body>
    <div id="app"></div>
  </body>
</html>
"#;

    let css_code = r#"
.container {
  display: flex;
}
@keyframes fadeIn {
  from { opacity: 0; }
}
"#;

    let sql_code = "CREATE TABLE users ( id INT );";
    let proto_code = r#"
syntax = "proto3";
service UserService {
    rpc GetUser (UserRequest) returns (UserResponse);
}
message UserRequest {
    int32 id = 1;
}
"#;

    let gql_code = r#"
type Query {
    me: User
}
type User {
    id: ID!
    name: String!
}
"#;

    let dockerfile_code = r#"
FROM alpine:latest AS builder
RUN apk add git
"#;

    let files = [
        ("src/math.zig", Lang::Zig, zig_code.as_bytes()),
        (
            "contracts/Token.sol",
            Lang::Solidity,
            solidity_code.as_bytes(),
        ),
        ("infra/main.tf", Lang::Hcl, hcl_code.as_bytes()),
        ("src/MainApp.scala", Lang::Scala, scala_code.as_bytes()),
        ("src/server.erl", Lang::Erlang, erlang_code.as_bytes()),
        ("index.html", Lang::Html, html_code.as_bytes()),
        ("styles.css", Lang::Css, css_code.as_bytes()),
        ("schema.sql", Lang::Sql, sql_code.as_bytes()),
        ("api.proto", Lang::Proto, proto_code.as_bytes()),
        ("schema.graphql", Lang::Graphql, gql_code.as_bytes()),
        ("Dockerfile", Lang::Dockerfile, dockerfile_code.as_bytes()),
    ];
    let interner = Interner::new();
    let registry = LanguageRegistry::new().unwrap();
    let ingest = ingest_in_memory_files(&files, &registry, &interner);
    let sym_names: Vec<String> = ingest
        .symbols
        .iter()
        .map(|s| interner.resolve(&s.name).to_string())
        .collect();

    assert!(sym_names.contains(&"add".to_string()));
    assert!(sym_names.contains(&"calculate".to_string()));
    assert!(sym_names.contains(&"Token".to_string()));
    assert!(sym_names.contains(&"transfer".to_string()));
    assert!(sym_names.contains(&"resource".to_string()));
    assert!(sym_names.contains(&"MainApp".to_string()));
    assert!(sym_names.contains(&"start".to_string()));
    assert!(sym_names.contains(&"UserService".to_string()));
    assert!(sym_names.contains(&"User".to_string()));
    assert!(sym_names.contains(&"builder".to_string()));
}

#[test]
fn test_objective_c_and_cpp_dialects() {
    let objc_code = r#"
#import "Renderer.h"

@protocol Renderable
- (void)render;
@end

@interface Renderer : NSObject <Renderable>
- (void)drawMesh;
@end

@implementation Renderer
- (void)drawMesh {
    [self render];
}
- (void)render {
    setup_pipeline();
}
@end

void setup_pipeline(void) {
    // C helper
}
"#;

    let objcpp_code = r#"
#import "Engine.h"

class CoreEngine {
public:
    void boot() {}
};

@implementation Controller
- (void)start {
    CoreEngine engine;
    engine.boot();
}
@end
"#;

    let files = [
        ("src/Renderer.m", Lang::ObjC, objc_code.as_bytes()),
        ("src/Controller.mm", Lang::ObjC, objcpp_code.as_bytes()),
    ];

    let interner = Interner::new();
    let registry = LanguageRegistry::new().unwrap();
    let ingest = ingest_in_memory_files(&files, &registry, &interner);

    let sym_names: Vec<String> = ingest
        .symbols
        .iter()
        .map(|s| interner.resolve(&s.name).to_string())
        .collect();

    assert!(sym_names.contains(&"Renderable".to_string()));
    assert!(sym_names.contains(&"Renderer".to_string()));
    assert!(sym_names.contains(&"drawMesh".to_string()));
    assert!(sym_names.contains(&"render".to_string()));
    assert!(sym_names.contains(&"setup_pipeline".to_string()));
    assert!(sym_names.contains(&"Controller".to_string()));
    assert!(sym_names.contains(&"start".to_string()));

    let graph = Graph::build(&ingest, &interner);
    assert!(graph.node_count() >= 5);
    assert!(graph.edge_count() >= 1);
}
