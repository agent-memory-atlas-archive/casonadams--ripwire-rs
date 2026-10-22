use ripwire_rs::{Graph, Interner, Lang, LanguageRegistry, NodeId, Symbol, ingest_in_memory_files};

const TASKS: &str = "export interface Task {\n  id: string;\n}\n\
export interface TaskEnqueuer {\n  enqueue(payload: Task): Promise<string>;\n}\n\
export class ReviewTaskQueue implements TaskEnqueuer {\n  async enqueue(payload: Task): Promise<string> {\n    return payload.id;\n  }\n}\n\
export class OtherQueue {\n  enqueue(payload: Task): number {\n    return payload.id.length;\n  }\n}\n";

const WEBHOOK: &str = "import type { Task, TaskEnqueuer } from \"../gcp/tasks\";\n\
export function acceptEvent(input: { queue: TaskEnqueuer }, task: Task): Promise<string> {\n  return input.queue.enqueue(task);\n}\n";

const CALLERS: &str = "import { ReviewTaskQueue, OtherQueue } from \"../gcp/tasks\";\n\
export function runDirect(): void {\n  const queue = new ReviewTaskQueue();\n  queue.enqueue({ id: \"a\" });\n}\n\
export function runOther(): void {\n  const other = new OtherQueue();\n  other.enqueue({ id: \"b\" });\n}\n\
export function makeQueue(): ReviewTaskQueue {\n  return new ReviewTaskQueue();\n}\n\
export function viaFactory(): void {\n  makeQueue().enqueue({ id: \"c\" });\n}\n\
export function viaUnannotated(): void {\n  unannotated().enqueue({ id: \"d\" });\n}\n\
function unannotated() {\n  return new ReviewTaskQueue();\n}\n";

struct Fixture {
    interner: Interner,
    ingest: ripwire_rs::IngestResult,
    graph: Graph,
}

fn fixture(files: &[(&str, &str)]) -> Fixture {
    let interner = Interner::new();
    let registry = LanguageRegistry::new().unwrap();
    let sources: Vec<(&str, Lang, &[u8])> = files
        .iter()
        .map(|(path, src)| (*path, Lang::TypeScript, src.as_bytes()))
        .collect();
    let ingest = ingest_in_memory_files(&sources, &registry, &interner);
    let graph = Graph::build(&ingest, &interner);
    Fixture {
        interner,
        ingest,
        graph,
    }
}

impl Fixture {
    /// The method `name` declared in class or interface `owner`.
    fn method(&self, owner: &str, name: &str) -> NodeId {
        self.symbol(owner, name).id
    }

    fn symbol(&self, owner: &str, name: &str) -> &Symbol {
        self.ingest
            .symbols
            .iter()
            .find(|sym| {
                self.interner.resolve(&sym.name) == name
                    && sym
                        .scope
                        .is_some_and(|scope| self.interner.resolve(&scope) == owner)
            })
            .unwrap_or_else(|| panic!("{owner}.{name} not indexed"))
    }

    fn caller_names(&self, target: NodeId) -> Vec<String> {
        let mut names: Vec<String> = self
            .graph
            .callers(target)
            .map(|(id, _)| {
                let sym = self.ingest.sym(id).expect("caller symbol");
                self.interner.resolve(&sym.name).to_string()
            })
            .collect();
        names.sort();
        names
    }
}

fn all_files() -> Fixture {
    fixture(&[
        ("src/gcp/tasks.ts", TASKS),
        ("src/service/webhook.ts", WEBHOOK),
        ("tests/tasks.test.ts", CALLERS),
    ])
}

#[test]
fn interface_typed_call_reaches_the_implementing_method() {
    let fx = all_files();
    let implementor = fx.method("ReviewTaskQueue", "enqueue");
    assert!(
        fx.caller_names(implementor)
            .contains(&"acceptEvent".to_string()),
        "{:?}",
        fx.caller_names(implementor)
    );
}

#[test]
fn unrelated_class_with_the_same_method_name_is_not_linked() {
    let fx = all_files();
    let unrelated = fx.method("OtherQueue", "enqueue");
    let callers = fx.caller_names(unrelated);
    assert!(!callers.contains(&"acceptEvent".to_string()), "{callers:?}");
    assert!(callers.contains(&"runOther".to_string()), "{callers:?}");
    let implementor = fx.caller_names(fx.method("ReviewTaskQueue", "enqueue"));
    assert!(
        !implementor.contains(&"runOther".to_string()),
        "{implementor:?}"
    );
}

#[test]
fn factory_with_explicit_return_annotation_reaches_the_class_method() {
    let fx = all_files();
    let callers = fx.caller_names(fx.method("ReviewTaskQueue", "enqueue"));
    assert!(callers.contains(&"viaFactory".to_string()), "{callers:?}");
    assert!(callers.contains(&"runDirect".to_string()), "{callers:?}");
}

#[test]
fn factory_without_return_annotation_is_not_guessed() {
    let fx = all_files();
    let mut all = Vec::new();
    for owner in ["ReviewTaskQueue", "OtherQueue", "TaskEnqueuer"] {
        all.extend(fx.caller_names(fx.method(owner, "enqueue")));
    }
    assert!(!all.contains(&"viaUnannotated".to_string()), "{all:?}");
}

#[test]
fn interface_declaration_stays_a_callee_of_the_interface_call() {
    let fx = all_files();
    let declared = fx.method("TaskEnqueuer", "enqueue");
    assert!(
        fx.caller_names(declared)
            .contains(&"acceptEvent".to_string()),
        "{:?}",
        fx.caller_names(declared)
    );
}
