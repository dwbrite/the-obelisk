use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use fold::pipeline::{terminal::Table, Filter, KeyBy};
use fold::stream::Stream;
use frontmatter_gen::extract;
use notify::{EventKind, RecursiveMode};
use notify_debouncer_full::{new_debouncer, DebounceEventResult, DebouncedEvent};
use serde::{Deserialize, Serialize};
use tauri::Manager;

const VAULT: &str = "../my-vault";

#[derive(Clone, Serialize, Deserialize, Debug)]
struct Note {
    path: String,
    title: String,
    tags: Vec<String>,
    published: bool,
}

fn has_tag(n: &Note, tag: &str) -> bool {
    n.tags.iter().any(|t| t == tag)
}
fn is_home(n: &Note) -> bool {
    has_tag(n, "home")
}
fn is_published_blog(n: &Note) -> bool {
    has_tag(n, "anm_blog") && n.published
}
fn is_void(n: &Note) -> bool {
    has_tag(n, "void")
}

struct Vault {
    stream: Stream<Note, Pipeline>,
    prev: HashMap<PathBuf, Note>,
}

fn open_vault(db: &Path) -> Vault {
    let _ = fs::remove_dir_all(db);
    Vault {
        stream: Stream::new(
            db,
            (
                view(is_home, "home_notes"),
                view(is_published_blog, "blog_notes"),
                view(is_void, "void_notes"),
            ),
        ),
        prev: HashMap::new(),
    }
}

type View = Filter<Note, fn(&Note) -> bool, KeyBy<fn(&Note) -> String, Table<String, Note>, String, Note>>;
type Pipeline = (View, View, View); // home, blog, void

fn view(pred: fn(&Note) -> bool, table: &str) -> View {
    Filter::new(
        pred,
        KeyBy::new(note_path as fn(&Note) -> String, Table::new(table)),
    )
}

///////////////////////////////////////////////////////////////////////////

fn has_home_tag(n: &Note) -> bool {
    n.tags.iter().any(|t| t == "home")
}
fn note_path(n: &Note) -> String {
    n.path.clone()
}

// ---------- parsing ----------

fn is_md(path: &Path) -> bool {
    path.extension().is_some_and(|e| e == "md")
}

fn parse_note(path: &Path) -> anyhow::Result<Note> {
    let markdown = fs::read_to_string(path)?;
    let (fm, body) = if markdown.starts_with("---") {
        match extract(&markdown) {
            Ok((fm, body)) => (Some(fm), body),
            Err(e) => {
                eprintln!("bad frontmatter in {}: {e}", path.display());
                (None, markdown.as_str())
            }
        }
    } else {
        (None, markdown.as_str())
    };

    let mut tags: Vec<String> = Vec::new();
    // frontmatter `tags:` — either a list or a single string
    if let Some(v) = fm.as_ref().and_then(|fm| fm.get("tags")) {
        if let Some(arr) = v.as_array() {
            tags.extend(arr.iter().filter_map(|t| t.as_str()).map(String::from));
        } else if let Some(s) = v.as_str() {
            tags.push(s.to_string());
        }
    }
    // inline #tags in the body
    tags.extend(
        body.split_whitespace()
            .filter_map(|w| w.strip_prefix('#'))
            .filter(|t| !t.is_empty() && t.chars().next().is_some_and(char::is_alphanumeric))
            .map(|t| t.trim_end_matches(|c: char| !c.is_alphanumeric()).to_string()),
    );
    tags.sort();
    tags.dedup();

    let title = fm
        .as_ref()
        .and_then(|fm| fm.get("title").and_then(|t| t.as_str().map(String::from)))
        .or_else(|| path.file_stem().map(|s| s.to_string_lossy().into_owned()))
        .unwrap_or_default();

    let published = fm
        .as_ref()
        .and_then(|fm| fm.get("published"))
        .is_some_and(|v| matches!(v, frontmatter_gen::Value::Boolean(true)));

    Ok(Note {
        path: path.to_string_lossy().into_owned(),
        title,
        tags,
        published,
    })
}

// ---------- indexing ----------

enum Op {
    Upsert(PathBuf, Note),
    Remove(PathBuf),
}

/// Apply a batch of ops as one atomic wtx: retract old versions, insert new.
fn apply(vault: &mut Vault, ops: Vec<Op>) {
    let mut changes = Vec::new(); // (old, new)
    for op in ops {
        match op {
            Op::Upsert(path, note) => {
                let old = vault.prev.insert(path, note.clone());
                changes.push((old, Some(note)));
            }
            Op::Remove(path) => {
                if let Some(old) = vault.prev.remove(&path) {
                    changes.push((Some(old), None));
                }
            }
        }
    }
    if changes.is_empty() {
        return;
    }
    vault.stream.wtx(|tx| {
        for (old, new) in &changes {
            if let Some(old) = old {
                tx.remove(old);
            }
            if let Some(new) = new {
                tx.insert(new);
            }
        }
    });
}

/// Hidden file/dir name: ".obsidian", ".trash", ".git", ".DS_Store", …
fn is_hidden_name(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.starts_with('.'))
}

/// For watcher events: any hidden component *below* the vault root.
/// (Checked against the relative path so a hidden dir in VAULT's own
/// ancestry wouldn't blacklist the whole vault.)
fn in_hidden_dir(path: &Path) -> bool {
    path.strip_prefix(VAULT)
        .unwrap_or(path)
        .components()
        .any(|c| {
            c.as_os_str()
                .to_str()
                .is_some_and(|s| s.starts_with('.'))
        })
}

fn collect_initial(dir: &Path, ops: &mut Vec<Op>) {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) => {
            eprintln!("can't read dir {}: {e}", dir.display());
            return; // skip this dir, keep walking the rest
        }
    };
    for entry in entries {
        let path = match entry {
            Ok(entry) => entry.path(),
            Err(e) => {
                eprintln!("bad dir entry in {}: {e}", dir.display());
                continue;
            }
        };
        if is_hidden_name(&path) {
            continue; // never descend into .obsidian, .trash, etc.
        }
        if path.is_dir() {
            collect_initial(&path, ops);
        } else if is_md(&path) {
            match parse_note(&path) {
                Ok(note) => ops.push(Op::Upsert(path, note)),
                Err(e) => eprintln!("skipping {}: {e}", path.display()),
            }
        }
    }
}

fn handle_events(vault: &Arc<Mutex<Vault>>, events: Vec<DebouncedEvent>) {
    let mut ops = Vec::new();
    for event in &events {
        for path in &event.paths {
            if !is_md(path) || in_hidden_dir(path) {
                continue;
            }
            match event.kind {
                EventKind::Create(_) | EventKind::Modify(_) if path.exists() => {
                    match parse_note(path) {
                        Ok(note) => ops.push(Op::Upsert(path.clone(), note)),
                        Err(e) => eprintln!("skipping {}: {e}", path.display()),
                    }


                }
                EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) => {
                    ops.push(Op::Remove(path.clone()));
                }
                _ => {}
            }
        }
    }
    apply(&mut vault.lock().unwrap(), ops);
}

#[tauri::command]
fn view_notes(view: String, vault: tauri::State<'_, Arc<Mutex<Vault>>>) -> Vec<Note> {
    vault.lock().unwrap().stream.rtx(|(home, blog, void)| {
        let table = match view.as_str() {
            "home" => home,
            "blog" => blog,
            "void" => void,
            _ => return Vec::new(),
        };
        table.iter().map(|(_, note)| note).collect()
    })
}

// ---------- tauri ----------

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir();
            println!("{:?}", data_dir);

            let db = app.path().app_data_dir()?.join("vault-index.db");
            let vault = Arc::new(Mutex::new(open_vault(&db)));
            app.manage(vault.clone());

            // Initial full index — synchronous, before the watcher starts.
            // A panic here is loud instead of swallowed.
            {
                let mut ops = Vec::new();
                collect_initial(Path::new(VAULT), &mut ops);
                let parsed = ops.len();
                let matched = ops
                    .iter()
                    .filter(|op| matches!(op, Op::Upsert(_, n) if has_home_tag(n)))
                    .count();

                let mut v = vault.lock().unwrap();
                apply(&mut v, ops);
                let (h, b, vd) = v.stream.rtx(|(home, blog, void)| {
                    (home.iter().count(), blog.iter().count(), void.iter().count())
                });
                println!("initial index: parsed={parsed} home={h} blog={b} void={vd}");
            }

            // Watcher starts only after the baseline exists — no more race
            // where an early edit event gets clobbered by a stale initial note.
            let watch_vault = vault.clone();
            let mut debouncer = new_debouncer(
                Duration::from_millis(300),
                None,
                move |res: DebounceEventResult| match res {
                    Ok(events) => handle_events(&watch_vault, events),
                    Err(errs) => errs.iter().for_each(|e| eprintln!("watch error: {e}")),
                },
            )?;
            debouncer.watch(Path::new(VAULT), RecursiveMode::Recursive)?;
            app.manage(debouncer);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![view_notes])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}