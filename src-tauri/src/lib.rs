// オブシリアンくん — second-brain Vault の番人
// Vault直下のCLAUDE.mdのルール（触ってはいけないフォルダ・命名規則）を前提にした読み取り＋Inbox振り分け。
use regex::Regex;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

fn vault_root() -> PathBuf {
    let home = std::env::var("HOME").expect("HOME not set");
    PathBuf::from(home)
        .join("Library/Mobile Documents/iCloud~md~obsidian/Documents/second-brain")
}

fn is_ignored(p: &Path) -> bool {
    p.components().any(|c| {
        let s = c.as_os_str().to_string_lossy();
        s == ".obsidian" || s == ".trash"
    })
}

fn md_files(root: &Path) -> Vec<PathBuf> {
    WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.into_path())
        .filter(|p| !is_ignored(p) && p.extension().map_or(false, |x| x == "md"))
        .collect()
}

fn rel(root: &Path, p: &Path) -> String {
    p.strip_prefix(root).unwrap_or(p).to_string_lossy().into_owned()
}

fn mtime_secs(p: &Path) -> u64 {
    fs::metadata(p)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// ---------- 俯瞰マップ ----------

#[derive(Serialize)]
struct FolderStat {
    name: String,
    count: usize,
    latest_file: String,
    latest_mtime: u64,
    protected: bool, // 自動ツールの書き込み先（動かしてはいけない）
}

#[derive(Serialize)]
struct RecentNote {
    path: String,
    mtime: u64,
}

#[derive(Serialize)]
struct Overview {
    total_md: usize,
    folders: Vec<FolderStat>,
    recent: Vec<RecentNote>,
}

const PROTECTED: [&str; 5] = [
    "wiki",
    "記事アーカイブ",
    "メディア文字起こし",
    "20_知識ネットワーク",
    "99_Archive",
];

#[tauri::command]
fn vault_overview() -> Result<Overview, String> {
    let root = vault_root();
    if !root.exists() {
        return Err(format!("Vaultが見つかりません: {}", root.display()));
    }
    let files = md_files(&root);
    let mut by_folder: HashMap<String, (usize, String, u64)> = HashMap::new();
    let mut all: Vec<(String, u64)> = Vec::with_capacity(files.len());
    for f in &files {
        let r = rel(&root, f);
        let top = r.split('/').next().unwrap_or("").to_string();
        let key = if r.contains('/') { top } else { "（ルート直下）".into() };
        let mt = mtime_secs(f);
        let e = by_folder.entry(key).or_insert((0, String::new(), 0));
        e.0 += 1;
        if mt > e.2 {
            e.1 = r.clone();
            e.2 = mt;
        }
        all.push((r, mt));
    }
    let mut folders: Vec<FolderStat> = by_folder
        .into_iter()
        .map(|(name, (count, latest_file, latest_mtime))| FolderStat {
            protected: PROTECTED.contains(&name.as_str()),
            name,
            count,
            latest_file,
            latest_mtime,
        })
        .collect();
    folders.sort_by(|a, b| b.count.cmp(&a.count));
    all.sort_by(|a, b| b.1.cmp(&a.1));
    let recent = all
        .into_iter()
        .take(30)
        .map(|(path, mtime)| RecentNote { path, mtime })
        .collect();
    Ok(Overview { total_md: files.len(), folders, recent })
}

// ---------- 健康診断 ----------

#[derive(Serialize)]
struct BrokenLink {
    source: String,
    target: String,
}

#[derive(Serialize)]
struct TagIssue {
    kind: String, // hash / trailing_underscore / case_collision
    tag: String,
    count: usize,
    detail: String,
}

#[derive(Serialize)]
struct Health {
    inbox_count: usize,
    broken_links: Vec<BrokenLink>,
    broken_links_total: usize,
    tag_issues: Vec<TagIssue>,
    naming_violations: Vec<String>,
    empty_files: Vec<String>,
}

#[tauri::command]
fn health_check() -> Result<Health, String> {
    let root = vault_root();
    let files = md_files(&root);
    let stems: HashSet<String> = files
        .iter()
        .filter_map(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
        .collect();

    let link_re = Regex::new(r"\[\[([^\]\|#]+)").unwrap();
    let tags_re = Regex::new(r"(?m)^tags:\s*\[(.*)\]\s*$").unwrap();
    let name_date_dup = Regex::new(r"_\d{8}\.md$").unwrap();

    let mut broken: Vec<BrokenLink> = Vec::new();
    let mut tag_counts: HashMap<String, usize> = HashMap::new();
    let mut naming: Vec<String> = Vec::new();
    let mut empty: Vec<String> = Vec::new();

    for f in &files {
        let r = rel(&root, f);
        let name = f.file_name().unwrap().to_string_lossy();
        // 命名ルール: スペース・全角スペース・末尾日付重複（保護フォルダ以外を対象）
        let top = r.split('/').next().unwrap_or("");
        if !PROTECTED.contains(&top) || top == "wiki" {
            if name.contains(' ') || name.contains('　') || name_date_dup.is_match(&name) {
                naming.push(r.clone());
            }
        }
        let Ok(text) = fs::read_to_string(f) else { continue };
        if text.trim().is_empty() {
            empty.push(r.clone());
        }
        for c in link_re.captures_iter(&text) {
            let t = c[1].trim();
            let base = t.rsplit('/').next().unwrap_or(t);
            if !base.is_empty() && !stems.contains(base) {
                broken.push(BrokenLink { source: r.clone(), target: t.to_string() });
            }
        }
        // frontmatter tags のみ
        if let Some(end) = text[3..].find("\n---") {
            let fm = &text[..end + 3];
            if let Some(m) = tags_re.captures(fm) {
                for t in m[1].split(',') {
                    let t = t.trim().trim_matches('"').trim_matches('\'');
                    if !t.is_empty() {
                        *tag_counts.entry(t.to_string()).or_insert(0) += 1;
                    }
                }
            }
        }
    }

    let mut tag_issues: Vec<TagIssue> = Vec::new();
    let mut by_lower: HashMap<String, Vec<(&String, usize)>> = HashMap::new();
    for (t, c) in &tag_counts {
        if t.starts_with('#') {
            tag_issues.push(TagIssue { kind: "hash".into(), tag: t.clone(), count: *c, detail: "タグに # が付いている".into() });
        }
        if t.ends_with('_') {
            tag_issues.push(TagIssue { kind: "trailing_underscore".into(), tag: t.clone(), count: *c, detail: "末尾に _ が残っている".into() });
        }
        by_lower.entry(t.to_lowercase()).or_default().push((t, *c));
    }
    for (_low, variants) in by_lower {
        if variants.len() > 1 {
            let names: Vec<String> = variants.iter().map(|(t, c)| format!("{t}({c})")).collect();
            let total: usize = variants.iter().map(|(_, c)| c).sum();
            tag_issues.push(TagIssue {
                kind: "case_collision".into(),
                tag: variants[0].0.clone(),
                count: total,
                detail: format!("大小文字ゆれ: {}", names.join(" / ")),
            });
        }
    }
    tag_issues.sort_by(|a, b| b.count.cmp(&a.count));

    let inbox_count = md_files(&root.join("00_Inbox")).len();
    let broken_links_total = broken.len();
    broken.truncate(100);
    naming.sort();
    empty.sort();

    Ok(Health { inbox_count, broken_links: broken, broken_links_total, tag_issues, naming_violations: naming, empty_files: empty })
}

// ---------- Inbox振り分け ----------

#[derive(Serialize)]
struct InboxItem {
    path: String,     // Vault相対
    name: String,
    mtime: u64,
    preview: String,  // 冒頭
}

#[tauri::command]
fn list_inbox() -> Result<Vec<InboxItem>, String> {
    let root = vault_root();
    let mut items: Vec<InboxItem> = md_files(&root.join("00_Inbox"))
        .into_iter()
        .map(|f| {
            let preview = fs::read_to_string(&f)
                .map(|t| t.chars().take(400).collect())
                .unwrap_or_default();
            InboxItem {
                path: rel(&root, &f),
                name: f.file_name().unwrap().to_string_lossy().into_owned(),
                mtime: mtime_secs(&f),
                preview,
            }
        })
        .collect();
    items.sort_by(|a, b| b.mtime.cmp(&a.mtime));
    Ok(items)
}

#[tauri::command]
fn list_dest_folders() -> Result<Vec<String>, String> {
    let root = vault_root();
    let mut dirs: Vec<String> = fs::read_dir(&root)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map_or(false, |t| t.is_dir()))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| !n.starts_with('.') && n != "00_Inbox" && n != "_meta" && n != "80_Assets")
        .collect();
    dirs.sort();
    Ok(dirs)
}

#[tauri::command]
fn read_note(path: String) -> Result<String, String> {
    let root = vault_root();
    let p = root.join(&path);
    if !p.starts_with(&root) || is_ignored(&p) {
        return Err("Vault外のパスは読めません".into());
    }
    fs::read_to_string(&p).map_err(|e| e.to_string())
}

/// Inboxのノートを移動する。ファイル名は変えないため [[リンク]] は壊れない。
/// 移動は _meta/整理ログ_オブシリアンくん.md に追記する。
#[tauri::command]
fn move_note(path: String, dest_folder: String) -> Result<String, String> {
    let root = vault_root();
    let src = root.join(&path);
    if !src.starts_with(root.join("00_Inbox")) {
        return Err("00_Inbox配下のノートのみ移動できます".into());
    }
    if dest_folder.contains("..") || dest_folder.starts_with('/') {
        return Err("不正な移動先です".into());
    }
    let name = src.file_name().ok_or("ファイル名が取れません")?.to_owned();
    let dest_dir = root.join(&dest_folder);
    if !dest_dir.is_dir() {
        return Err(format!("移動先フォルダがありません: {dest_folder}"));
    }
    let dest = dest_dir.join(&name);
    if dest.exists() {
        return Err(format!("移動先に同名ファイルがあります: {}", rel(&root, &dest)));
    }
    fs::rename(&src, &dest).map_err(|e| e.to_string())?;

    let log = root.join("_meta/整理ログ_オブシリアンくん.md");
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M");
    let mut body = fs::read_to_string(&log)
        .unwrap_or_else(|_| "# オブシリアンくん 振り分けログ\n\n| 日時 | 元 | 先 |\n|---|---|---|\n".into());
    body.push_str(&format!("| {now} | {} | {} |\n", path, rel(&root, &dest)));
    fs::write(&log, body).map_err(|e| format!("移動は成功しましたがログ書き込みに失敗: {e}"))?;
    Ok(rel(&root, &dest))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            vault_overview,
            health_check,
            list_inbox,
            list_dest_folders,
            read_note,
            move_note
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
