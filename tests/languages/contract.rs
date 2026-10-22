use ripwire_rs::Lang;
use ripwire_rs::ingest::classify_capture;
use ripwire_rs::languages::{ALL_LANGS, LanguageSemantics};
use std::collections::{HashMap, HashSet};

fn effective_extensions(sem: &'static dyn LanguageSemantics) -> Vec<&'static str> {
    let exts = sem.extensions();
    if exts.is_empty() {
        vec![sem.tag()]
    } else {
        exts.to_vec()
    }
}

#[test]
fn language_tags_are_unique_and_non_empty() {
    let mut seen = HashSet::new();
    for lang in ALL_LANGS {
        let tag = lang.semantics().tag();
        assert!(!tag.is_empty(), "{lang:?} has an empty tag");
        assert!(seen.insert(tag), "{lang:?} repeats tag {tag}");
    }
}

#[test]
fn extensions_belong_to_one_language_or_one_dialect_group() {
    let mut owner: HashMap<&str, Lang> = HashMap::new();
    for lang in ALL_LANGS {
        let sem = lang.semantics();
        for ext in effective_extensions(sem) {
            if let Some(&other) = owner.get(ext) {
                assert_eq!(
                    sem.dialect_group(),
                    other.semantics().dialect_group(),
                    "extension .{ext} is claimed by {other:?} and {lang:?}"
                );
            }
            owner.insert(ext, lang);
        }
    }
}

#[test]
fn every_declared_grammar_loads_with_known_captures() {
    let mut loaded = 0;
    for lang in ALL_LANGS {
        if let Some((_, query)) = lang.semantics().grammar_and_query() {
            loaded += 1;
            for capture in query.capture_names() {
                assert!(
                    classify_capture(capture).is_some(),
                    "{lang:?} tags.scm uses unknown capture @{capture}"
                );
            }
        }
    }
    assert!(loaded > 0, "no language declares a grammar");
}

#[test]
fn optional_layout_hooks_come_in_pairs() {
    for lang in ALL_LANGS {
        let sem = lang.semantics();
        assert_eq!(
            sem.layout_model().is_some(),
            sem.supports_c_layout(),
            "{lang:?} layout_model and supports_c_layout disagree"
        );
    }
}

#[test]
fn partner_subject_is_none_for_an_empty_stem() {
    for lang in ALL_LANGS.into_iter().chain([Lang::Unknown]) {
        let sem = lang.semantics();
        assert!(sem.test_partner_subject("", false).is_none(), "{lang:?}");
        assert!(sem.test_partner_subject("", true).is_none(), "{lang:?}");
    }
    assert!(!Lang::Unknown.semantics().supports_test_partners());
}
