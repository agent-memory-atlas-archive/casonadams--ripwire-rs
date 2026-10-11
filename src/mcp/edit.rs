use crate::domain::IngestResult;
use serde_json::{json, Value};
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Atomic edit operation type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditOp {
    ReplaceBody,
    InsertBefore,
    InsertAfter,
}

fn check_symlink(path: &Path) -> Result<(), Value> {
    if let Ok(meta) = fs::symlink_metadata(path) {
        if meta.file_type().is_symlink() {
            return Err(json!({
                "applied": false,
                "error": format!("refusing to edit '{}': target is a symlink", path.display())
            }));
        }
    }
    Ok(())
}

fn resolve_edit_target<'a>(
    ingest: &'a IngestResult,
    interner: &'a crate::domain::Interner,
    symbol_name: &str,
    path_hint: Option<&str>,
) -> Result<(&'a crate::domain::Symbol, &'a str), Value> {
    let mut matches = Vec::new();

    for sym in &ingest.symbols {
        let name = interner.resolve(&sym.name);
        if name == symbol_name {
            if let Some(file_info) = ingest.files.get(sym.file_id as usize) {
                let file_path = interner.resolve(&file_info.path);
                if path_hint.is_none() || path_hint.is_some_and(|h| file_path.contains(h)) {
                    matches.push((sym, file_path));
                }
            }
        }
    }

    if matches.is_empty() {
        return Err(json!({
            "applied": false,
            "error": format!("symbol '{symbol_name}' not found")
        }));
    }

    if matches.len() > 1 {
        let candidates: Vec<String> = matches
            .iter()
            .map(|(s, p)| format!("{p}:{}", s.line))
            .collect();
        return Err(json!({
            "applied": false,
            "error": format!("symbol '{symbol_name}' is ambiguous ({} definitions). Disambiguate with 'path'", matches.len()),
            "candidates": candidates
        }));
    }

    Ok(matches[0])
}

fn write_atomic_temp_file(target_path: &Path, content: &[u8]) -> std::io::Result<()> {
    let parent = target_path.parent().unwrap_or_else(|| Path::new("."));
    let filename = target_path
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("file");
    let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let temp_name = format!(".{filename}.tmp.{}.{counter}", std::process::id());
    let temp_path = parent.join(temp_name);

    {
        let mut f = File::create(&temp_path)?;
        f.write_all(content)?;
        f.sync_all()?;
    }

    fs::rename(&temp_path, target_path)?;
    Ok(())
}

fn splice_file_bytes(
    original: &[u8],
    op: EditOp,
    sig_start: usize,
    end_byte: usize,
    payload: &[u8],
) -> Result<(Vec<u8>, usize, usize), Value> {
    let len = original.len();
    if sig_start > len || end_byte > len || sig_start > end_byte {
        return Err(json!({
            "applied": false,
            "error": format!("invalid symbol span [{sig_start}, {end_byte}) for file size {len}")
        }));
    }

    let mut out = Vec::with_capacity(len + payload.len());
    let new_start: usize;
    let new_end: usize;

    match op {
        EditOp::ReplaceBody => {
            out.extend_from_slice(&original[..sig_start]);
            new_start = out.len();
            out.extend_from_slice(payload);
            new_end = out.len();
            out.extend_from_slice(&original[end_byte..]);
        }
        EditOp::InsertBefore => {
            out.extend_from_slice(&original[..sig_start]);
            new_start = out.len();
            out.extend_from_slice(payload);
            new_end = out.len();
            out.extend_from_slice(&original[sig_start..]);
        }
        EditOp::InsertAfter => {
            out.extend_from_slice(&original[..end_byte]);
            new_start = out.len();
            out.extend_from_slice(payload);
            new_end = out.len();
            out.extend_from_slice(&original[end_byte..]);
        }
    }

    Ok((out, new_start, new_end))
}

/// Perform an atomic symbol edit (replace, insert_before, or insert_after).
fn validate_payload(payload: &str) -> Result<(), Value> {
    if payload.contains('\0') {
        Err(json!({
            "applied": false,
            "error": "payload contains NUL byte; refusing binary write"
        }))
    } else {
        Ok(())
    }
}

fn validate_and_read_target(target_path: &Path) -> Result<Vec<u8>, Value> {
    check_symlink(target_path)?;
    fs::read(target_path).map_err(|e| {
        json!({
            "applied": false,
            "error": format!("cannot read file '{}': {e}", target_path.display())
        })
    })
}

/// Perform an atomic symbol edit (replace, insert_before, or insert_after).
pub fn perform_atomic_edit(
    ingest: &IngestResult,
    interner: &crate::domain::Interner,
    op: EditOp,
    symbol_name: &str,
    path_hint: Option<&str>,
    payload: &str,
) -> Value {
    if let Err(err) = validate_payload(payload) {
        return err;
    }

    let (symbol, file_rel_path) =
        match resolve_edit_target(ingest, interner, symbol_name, path_hint) {
            Ok(t) => t,
            Err(err) => return err,
        };

    let target_path = Path::new(file_rel_path);
    let original_bytes = match validate_and_read_target(target_path) {
        Ok(b) => b,
        Err(err) => return err,
    };

    let sig_start = symbol.sig_start_byte as usize;
    let end_byte = symbol.end_byte as usize;

    let (new_bytes, new_start, new_end) =
        match splice_file_bytes(&original_bytes, op, sig_start, end_byte, payload.as_bytes()) {
            Ok(res) => res,
            Err(err) => return err,
        };
    if let Err(e) = write_atomic_temp_file(target_path, &new_bytes) {
        return json!({
            "applied": false,
            "error": format!("failed to atomically write file '{}': {e}", target_path.display())
        });
    }

    json!({
        "applied": true,
        "file": file_rel_path,
        "symbol": symbol_name,
        "old_span": [sig_start, end_byte],
        "new_span": [new_start, new_end],
        "bytes_written": payload.len()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_splice_file_bytes_replace() {
        let original = b"fn old_function() { return 1; }\n";
        let payload = b"fn new_function() { return 2; }";
        let (out, new_start, new_end) =
            splice_file_bytes(original, EditOp::ReplaceBody, 0, 31, payload).unwrap();
        assert_eq!(new_start, 0);
        assert_eq!(new_end, payload.len());
        assert_eq!(out, b"fn new_function() { return 2; }\n");
    }

    #[test]
    fn test_splice_file_bytes_insert_before() {
        let original = b"fn foo() {}\n";
        let payload = b"// comment\n";
        let (out, new_start, new_end) =
            splice_file_bytes(original, EditOp::InsertBefore, 0, 11, payload).unwrap();
        assert_eq!(new_start, 0);
        assert_eq!(new_end, payload.len());
        assert_eq!(out, b"// comment\nfn foo() {}\n");
    }

    #[test]
    fn test_splice_file_bytes_insert_after() {
        let original = b"fn foo() {}\n";
        let payload = b"\nfn bar() {}\n";
        let (out, new_start, new_end) =
            splice_file_bytes(original, EditOp::InsertAfter, 0, 11, payload).unwrap();
        assert_eq!(new_start, 11);
        assert_eq!(new_end, 11 + payload.len());
        assert_eq!(out, b"fn foo() {}\nfn bar() {}\n\n");
    }

    #[test]
    fn test_perform_atomic_edit_refusals() {
        let interner = crate::domain::Interner::new();
        let ingest = crate::domain::IngestResult::new();

        let res_nul = perform_atomic_edit(
            &ingest,
            &interner,
            EditOp::ReplaceBody,
            "sym",
            None,
            "fn foo\0() {}",
        );
        assert_eq!(res_nul["applied"], false);

        let res_missing = perform_atomic_edit(
            &ingest,
            &interner,
            EditOp::ReplaceBody,
            "missing_symbol",
            None,
            "fn foo() {}",
        );
        assert_eq!(res_missing["applied"], false);

        assert!(validate_payload("valid code").is_ok());
        assert!(validate_payload("invalid \0 code").is_err());
        assert!(validate_and_read_target(Path::new("nonexistent.rs")).is_err());
    }
}
