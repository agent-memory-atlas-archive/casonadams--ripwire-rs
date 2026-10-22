use ripwire_rs::metrics::nonlocal::{NonlocalFunction, find_nonlocal_state};
use ripwire_rs::{Graph, Interner, Lang, LanguageRegistry, ingest_in_memory_files};

const CONFIG: &str = "src/config.ts";

fn state_report(files: &[(&str, &str)]) -> Vec<NonlocalFunction> {
    let interner = Interner::new();
    let registry = LanguageRegistry::new().unwrap();
    let sources: Vec<(&str, Lang, &[u8])> = files
        .iter()
        .map(|(path, src)| (*path, Lang::TypeScript, src.as_bytes()))
        .collect();
    let ingest = ingest_in_memory_files(&sources, &registry, &interner);
    let graph = Graph::build(&ingest, &interner);
    find_nonlocal_state(&ingest, &graph, &interner)
}

fn function<'a>(report: &'a [NonlocalFunction], name: &str) -> Option<&'a NonlocalFunction> {
    report.iter().find(|f| f.sym_name == name)
}

const SCALARS: &str = "export const GREP_TIMEOUT_MS = 30_000;
export const LABEL = \"x\";
export const ON = true;
export const NOTHING = null;
export const NEGATIVE = -1;
";

const READS_SCALARS: &str =
    "import { GREP_TIMEOUT_MS, LABEL, ON, NOTHING, NEGATIVE } from \"./config\";
export function runSearch(): number {
  return GREP_TIMEOUT_MS + (ON ? 1 : 0) + NEGATIVE;
}
export function label(): string {
  return LABEL + String(NOTHING);
}
";

#[test]
fn reads_of_const_scalars_are_not_mutable_state() {
    let report = state_report(&[(CONFIG, SCALARS), ("src/use.ts", READS_SCALARS)]);
    assert!(
        report.is_empty(),
        "immutable scalar reads reported as state: {:?}",
        report.iter().map(|f| &f.sym_name).collect::<Vec<_>>()
    );
}

const MODULE_LET: &str = "let depth = 0;
var legacy = 1;
export let exported = \"a\";
export function current(): number {
  return depth;
}
";

const WRITES_LET: &str = "import { depth, legacy, exported } from \"./config\";
export function descend(): void {
  depth = depth + 1;
}
export function touchLegacy(): void {
  legacy++;
  exported += \"b\";
}
";

#[test]
fn module_level_let_and_var_are_mutable_state_with_writes() {
    let report = state_report(&[(CONFIG, MODULE_LET), ("src/use.ts", WRITES_LET)]);
    let descend = function(&report, "descend").expect("descend touches depth");
    assert_eq!((descend.reads, descend.writes), (1, 1));
    assert!(descend.cells.iter().all(|c| c.cell_name == "depth"));
    let touch = function(&report, "touchLegacy").expect("touchLegacy writes module vars");
    assert_eq!(touch.writes, 2);
    let current = function(&report, "current").expect("read of let is state");
    assert_eq!((current.reads, current.writes), (1, 0));
}

const MUTABLE_OBJECT: &str = "export const LIMITS = { max: 10 };
export const TAGS = [\"a\"];
";

const MUTATES_OBJECT: &str = "import { LIMITS, TAGS } from \"./config\";
export function raise(): void {
  LIMITS.max = 3;
}
export function tag(): void {
  TAGS[0] = \"b\";
}
export function readMax(): number {
  return LIMITS.max;
}
";

#[test]
fn mutation_through_const_object_reference_is_a_write() {
    let report = state_report(&[(CONFIG, MUTABLE_OBJECT), ("src/use.ts", MUTATES_OBJECT)]);
    let raise = function(&report, "raise").expect("member write through const is state");
    assert_eq!((raise.reads, raise.writes), (0, 1));
    assert_eq!(raise.cells[0].cell_name, "LIMITS");
    assert!(raise.cells[0].is_write);
    let tag = function(&report, "tag").expect("subscript write through const is state");
    assert_eq!(tag.writes, 1);
    let read = function(&report, "readMax").expect("reading const object contents is state");
    assert_eq!((read.reads, read.writes), (1, 0));
}

const SHADOWED: &str = "let count = 0;
";

const LOCAL_WRITES: &str = "import { count } from \"./config\";
export function localOnly(): number {
  let count = 0;
  count = count + 1;
  return count;
}
export function param(count: { n: number }): void {
  count.n = 2;
}
";

#[test]
fn writes_to_local_bindings_do_not_reach_module_state() {
    let report = state_report(&[(CONFIG, SHADOWED), ("src/use.ts", LOCAL_WRITES)]);
    assert!(
        function(&report, "localOnly").is_none(),
        "local let shadowing a module let is not module state"
    );
    assert!(
        function(&report, "param").is_none(),
        "parameter mutation is not module state"
    );
}

const DEV_TOKEN_SCRIPT: &str = "let json;
export function refreshToken(raw: string): string {
  json = JSON.parse(raw);
  return json.token;
}
";

const INTERCEPT_SPEC: &str =
    "export async function intercept(res: { json(): Promise<{ data: number }> }): Promise<number> {
  const json = await res.json();
  return json.data;
}
export async function bare(res: { json(): Promise<{ data: number }> }): Promise<number> {
  return (await res.json()).data;
}
";

#[test]
fn local_declaration_shadows_cross_file_global_of_same_name() {
    let report = state_report(&[
        ("scripts/refresh-dev-token.ts", DEV_TOKEN_SCRIPT),
        ("e2e/intercept.spec.ts", INTERCEPT_SPEC),
    ]);
    assert!(
        function(&report, "intercept").is_none(),
        "a local const json and a property call res.json() bind to another file's let json"
    );
    assert!(
        function(&report, "bare").is_none(),
        "a property call on an unknown receiver is not a read of a same-named variable"
    );
}

const ENV_CONFIG: &str = "export const OWNER = process.env.DEV_MEMBER_ID!;
export const BASE = `${process.env.HOST}/api`;
export const ENDPOINT = process.env.HOST + \"/graphql\";
export const FALLBACK = process.env.PORT ?? \"3000\";
export const LIMITS = { max: process.env.MAX };
export let counter = 0;
";

const ENV_USE: &str =
    "import { OWNER, BASE, ENDPOINT, FALLBACK, LIMITS, counter } from \"./config\";
export function owner(): string {
  return OWNER + BASE + ENDPOINT + FALLBACK;
}
export function limit(): unknown {
  return LIMITS.max;
}
export function bump(): void {
  counter++;
}
";

#[test]
fn const_bound_to_environment_scalar_is_not_mutable_state() {
    let report = state_report(&[(CONFIG, ENV_CONFIG), ("src/use.ts", ENV_USE)]);
    assert!(
        function(&report, "owner").is_none(),
        "env-derived scalar consts are immutable: {:?}",
        function(&report, "owner").map(|f| &f.cells)
    );
    let limit = function(&report, "limit").expect("const object contents stay state reads");
    assert_eq!((limit.reads, limit.writes), (1, 0));
    let bump = function(&report, "bump").expect("genuine let write stays state");
    assert_eq!((bump.reads, bump.writes), (0, 1));
}
