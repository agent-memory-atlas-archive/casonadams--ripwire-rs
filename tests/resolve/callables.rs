use ripwire_rs::{
    EdgeProv, Interner, Lang, LanguageRegistry, ingest_in_memory_files, resolve_all_references,
};

fn targets(source: &str, caller: &str) -> Vec<(String, EdgeProv)> {
    let registry = LanguageRegistry::new().unwrap();
    let interner = Interner::new();
    let ingest = ingest_in_memory_files(
        &[("src/dispatch.rs", Lang::Rust, source.as_bytes())],
        &registry,
        &interner,
    );
    let resolution = resolve_all_references(&ingest, &interner);
    let caller = ingest
        .symbols
        .iter()
        .find(|symbol| interner.resolve(&symbol.name) == caller)
        .unwrap()
        .id;
    let mut targets: Vec<_> = resolution
        .edge_accumulators
        .iter()
        .filter(|((from, _), _)| *from == caller)
        .map(|((_, to), edge)| {
            let name = interner.resolve(&ingest.symbols[*to as usize].canonical_id);
            (
                name.strip_prefix("src/dispatch.rs::")
                    .unwrap_or(name)
                    .to_owned(),
                edge.prov,
            )
        })
        .collect();
    targets.sort();
    targets
}

#[test]
fn callable_fields_follow_assignments_to_the_receiver_type_only() {
    let source = r#"
struct Command { handler: fn(u32) }
struct Other { handler: fn(u32) }
fn first(value: u32) {}
fn second(value: u32) {}
fn unrelated(value: u32) {}
static COMMANDS: &[Command] = &[Command { handler: first }, Command { handler: second }];
static OTHER: Other = Other { handler: unrelated };
fn invoke(command: &Command) { (command.handler)(1); }
"#;
    assert_eq!(
        targets(source, "invoke"),
        [
            ("first".into(), EdgeProv::Split),
            ("second".into(), EdgeProv::Split)
        ]
    );
}

#[test]
fn storing_a_callback_does_not_call_it() {
    let source = r#"
struct Command { handler: fn() }
fn action() {}
fn construct_only() -> Command { Command { handler: action } }
"#;
    let reached = targets(source, "construct_only");
    assert!(
        reached.iter().all(|(name, _)| name != "action"),
        "{reached:?}"
    );
}

#[test]
fn local_macro_projects_path_arguments_into_callable_fields() {
    let source = r#"
type Handler = fn();
struct Command { handler: Handler }
macro_rules! command {
    ($label:literal, $handler:path) => { Command { handler: $handler } };
}
mod actions { pub fn first() {} pub fn second() {} }
static COMMANDS: &[Command] = &[
    command!("first, label", actions::first),
    command!("second", /* argument */ actions :: second),
];
fn invoke(command: &Command) { (command.handler)(); }
"#;
    assert_eq!(
        targets(source, "invoke"),
        [
            ("actions::first".into(), EdgeProv::Split),
            ("actions::second".into(), EdgeProv::Split)
        ]
    );
}

#[test]
fn an_unmodeled_callable_field_does_not_fall_back_to_a_same_named_function() {
    let source = r#"
struct Command { handler: fn() }
fn handler() {}
fn action() {}
macro_rules! command {
    ($handler:path) => { Command { handler: $handler } };
    () => { Command { handler: action } };
}
static COMMAND: Command = command!(action);
fn invoke(command: &Command) { (command.handler)(); }
"#;
    assert!(
        !targets(source, "invoke")
            .iter()
            .any(|(name, _)| name == "handler")
    );
}

#[test]
fn repeated_macro_patterns_do_not_invent_callable_assignments() {
    let source = r#"
struct Command { handler: fn() }
fn handler() {}
fn action() {}
macro_rules! commands { ($($handler:path),*) => { [$(Command { handler: $handler }),*] }; }
static COMMANDS: &[Command] = &commands!(action);
fn invoke(command: &Command) { (command.handler)(); }
"#;
    assert!(
        !targets(source, "invoke")
            .iter()
            .any(|(name, _)| name == "handler")
    );
}

#[test]
fn duplicate_local_type_names_do_not_merge_callable_fields() {
    let source = r#"
mod first { pub struct Command { pub handler: fn() } pub fn action() {} pub static COMMAND: Command = Command { handler: action }; }
mod second { pub struct Command { pub handler: fn() } pub fn unrelated() {} pub static COMMAND: Command = Command { handler: unrelated }; }
fn invoke(command: &first::Command) { (command.handler)(); }
"#;
    assert!(
        !targets(source, "invoke")
            .iter()
            .any(|(name, _)| name == "unrelated")
    );
}

#[test]
fn an_external_receiver_does_not_take_a_local_callable_field_with_the_same_type_name() {
    let source = r#"
struct Command { handler: fn() }
fn action() {}
static COMMAND: Command = Command { handler: action };
fn invoke(command: &external::Command) { (command.handler)(); }
"#;
    assert_eq!(targets(source, "invoke"), []);
}

#[test]
fn callable_target_arity_is_checked_at_the_invocation() {
    let source = r#"
struct Command { handler: fn(u32) }
fn correct(value: u32) {}
fn incompatible() {}
static COMMANDS: &[Command] = &[Command { handler: correct }, Command { handler: incompatible }];
fn invoke(command: &Command) { (command.handler)(1); }
"#;
    assert_eq!(
        targets(source, "invoke"),
        [("correct".into(), EdgeProv::Binding)]
    );
}

#[test]
fn callback_assignments_resolve_in_their_lexical_module() {
    let source = r#"
struct Command { handler: fn() }
mod first {
    use super::Command;
    fn action() {}
    static COMMAND: Command = Command { handler: action };
}
mod second { fn action() {} }
fn invoke(command: &Command) { (command.handler)(); }
"#;
    assert_eq!(
        targets(source, "invoke"),
        [("first::action".into(), EdgeProv::Binding)]
    );
}

#[test]
fn unknown_receivers_do_not_pick_the_only_populated_callable_field() {
    let source = r#"
struct Command { handler: fn() }
struct Other { handler: fn() }
fn action() {}
static COMMAND: Command = Command { handler: action };
fn invoke() { let value = external::source(); (value.handler)(); }
"#;
    assert!(
        !targets(source, "invoke")
            .iter()
            .any(|(name, _)| name == "action")
    );
}

#[test]
fn external_callback_paths_do_not_bind_same_named_local_functions() {
    let source = r#"
struct Command { handler: fn() }
fn action() {}
static COMMAND: Command = Command { handler: external::action };
fn invoke(command: &Command) { (command.handler)(); }
"#;
    assert_eq!(targets(source, "invoke"), []);
}
