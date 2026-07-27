use base64::{engine::general_purpose, Engine as _};
use serde::Serialize;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Serialize)]
struct Entry {
    name: String,
    path: String,
    is_dir: bool,
    is_image: bool,
    has_docs: bool,
}

const SKIP: [&str; 13] = [
    "node_modules", ".git", "target", "dist", "vendor", "venv", ".venv",
    "__pycache__", ".next", "build", ".idea", ".vscode", ".DS_Store",
];

fn is_image(p: &Path) -> bool {
    matches!(
        p.extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_lowercase())
            .as_deref(),
        Some("png" | "jpg" | "jpeg" | "gif" | "svg" | "webp" | "bmp")
    )
}

fn is_doc_file(name: &str) -> bool {
    let l = name.to_lowercase();
    l.ends_with(".md") || l.ends_with(".markdown") || l.ends_with(".txt") || l.ends_with(".pdf")
        || l.ends_with(".html") || l.ends_with(".htm")
        || l.ends_with(".png") || l.ends_with(".jpg") || l.ends_with(".jpeg")
        || l.ends_with(".gif") || l.ends_with(".svg") || l.ends_with(".webp") || l.ends_with(".bmp")
}

/// ¿La carpeta contiene algún doc (recursivo, acotado)? Revisa archivos primero para salir rápido.
fn dir_has_docs(path: &Path, depth: u32) -> bool {
    if depth == 0 {
        return false;
    }
    let entries: Vec<_> = match fs::read_dir(path) {
        Ok(e) => e.flatten().collect(),
        Err(_) => return false,
    };
    for entry in &entries {
        if entry.path().is_file() {
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.starts_with('.') && is_doc_file(&name) {
                return true;
            }
        }
    }
    for entry in &entries {
        let p = entry.path();
        if p.is_dir() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') || SKIP.contains(&name.as_str()) {
                continue;
            }
            if dir_has_docs(&p, depth - 1) {
                return true;
            }
        }
    }
    false
}

/// Carpeta raíz por defecto.
#[tauri::command]
fn default_root() -> String {
    match std::env::var("HOME") {
        Ok(home) => {
            let gh = format!("{}/Documents/GitHub", home);
            if Path::new(&gh).is_dir() { gh } else { home }
        }
        Err(_) => String::new(),
    }
}

/// Lista carpetas + archivos de una ruta (carpetas primero).
#[tauri::command]
fn list_dir(path: String) -> Result<Vec<Entry>, String> {
    let mut out = Vec::new();
    for entry in fs::read_dir(&path).map_err(|e| e.to_string())?.flatten() {
        let p = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        let is_dir = p.is_dir();
        if is_dir && SKIP.contains(&name.as_str()) {
            continue;
        }
        out.push(Entry {
            name,
            path: p.to_string_lossy().to_string(),
            is_dir,
            is_image: is_image(&p),
            has_docs: if is_dir { dir_has_docs(&p, 6) } else { false },
        });
    }
    out.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });
    Ok(out)
}

#[tauri::command]
fn read_file(path: String) -> Result<String, String> {
    fs::read_to_string(&path).map_err(|e| e.to_string())
}

#[tauri::command]
fn write_file(path: String, content: String) -> Result<(), String> {
    if let Some(parent) = Path::new(&path).parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(&path, content).map_err(|e| e.to_string())
}

#[tauri::command]
fn create_dir(path: String) -> Result<(), String> {
    fs::create_dir_all(&path).map_err(|e| e.to_string())
}

/// Lee una imagen y la devuelve como data URI (base64) para mostrarla en la app.
#[tauri::command]
fn read_image_data_uri(path: String) -> Result<String, String> {
    let bytes = fs::read(&path).map_err(|e| e.to_string())?;
    let ext = Path::new(&path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    let mime = match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "pdf" => "application/pdf",
        _ => "application/octet-stream",
    };
    let b64 = general_purpose::STANDARD.encode(bytes);
    Ok(format!("data:{};base64,{}", mime, b64))
}

/// Copia un archivo (ej. una imagen arrastrada) dentro de una carpeta.
#[tauri::command]
fn copy_into(src: String, dest_dir: String) -> Result<String, String> {
    let name = Path::new(&src)
        .file_name()
        .ok_or_else(|| "archivo inválido".to_string())?
        .to_string_lossy()
        .to_string();
    let dest = Path::new(&dest_dir).join(&name);
    fs::copy(&src, &dest).map_err(|e| e.to_string())?;
    Ok(dest.to_string_lossy().to_string())
}

/// Elimina un archivo, o una carpeta con todo su contenido.
#[tauri::command]
fn delete_path(path: String) -> Result<(), String> {
    let p = Path::new(&path);
    if p.is_dir() {
        fs::remove_dir_all(p).map_err(|e| e.to_string())
    } else {
        fs::remove_file(p).map_err(|e| e.to_string())
    }
}

#[derive(Serialize)]
struct FoundDoc {
    file: String,
    path: String,
    rel: String,
}

/// Matchea por nombre exacto, o por extensión si el patrón es "*.ext" o ".ext".
fn matches_pattern(name: &str, pat_lower: &str) -> bool {
    let n = name.to_lowercase();
    if let Some(ext) = pat_lower.strip_prefix("*.") {
        n.rsplit('.').next() == Some(ext)
    } else if let Some(ext) = pat_lower.strip_prefix('.') {
        n.rsplit('.').next() == Some(ext)
    } else {
        n == pat_lower
    }
}

fn find_docs_rec(base: &Path, dir: &Path, names: &[String], out: &mut Vec<FoundDoc>, depth: u32) {
    if depth == 0 {
        return;
    }
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let p = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        if p.is_dir() {
            if SKIP.contains(&name.as_str()) {
                continue;
            }
            find_docs_rec(base, &p, names, out, depth - 1);
        } else if names.iter().any(|pat| matches_pattern(&name, pat)) {
            let rel = p
                .parent()
                .and_then(|par| par.strip_prefix(base).ok())
                .map(|r| r.to_string_lossy().to_string())
                .unwrap_or_default();
            out.push(FoundDoc {
                file: name,
                path: p.to_string_lossy().to_string(),
                rel,
            });
        }
    }
}

/// Busca recursivamente todos los archivos que coincidan con los nombres (README.md, SECURITY.md…).
#[tauri::command]
fn find_docs(project_dir: String, names: Vec<String>) -> Result<Vec<FoundDoc>, String> {
    let lower: Vec<String> = names.iter().map(|n| n.to_lowercase()).collect();
    let base = Path::new(&project_dir);
    let mut out = Vec::new();
    find_docs_rec(base, base, &lower, &mut out, 8);
    out.sort_by(|a, b| a.rel.to_lowercase().cmp(&b.rel.to_lowercase()));
    Ok(out)
}

fn tree_rec(dir: &Path, prefix: &str, out: &mut String, depth: u32) {
    if depth == 0 || out.len() > 7000 {
        return;
    }
    let mut entries: Vec<_> = match fs::read_dir(dir) {
        Ok(e) => e.flatten().collect(),
        Err(_) => return,
    };
    entries.sort_by_key(|e| e.file_name());
    for entry in &entries {
        if out.len() > 7000 {
            out.push_str(prefix);
            out.push_str("…\n");
            return;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        let p = entry.path();
        let is_dir = p.is_dir();
        if is_dir && SKIP.contains(&name.as_str()) {
            continue;
        }
        out.push_str(prefix);
        out.push_str(&name);
        if is_dir {
            out.push('/');
        }
        out.push('\n');
        if is_dir {
            tree_rec(&p, &format!("{}  ", prefix), out, depth - 1);
        }
    }
}

/// Árbol de texto del proyecto (para dar contexto a una IA).
#[tauri::command]
fn project_tree(path: String) -> String {
    let mut out = String::new();
    tree_rec(Path::new(&path), "", &mut out, 4);
    out
}

fn collect_docs(dir: &Path, ignored: &[String], out: &mut Vec<PathBuf>, depth: u32) {
    if depth == 0 {
        return;
    }
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let p = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        if ignored.iter().any(|ig| ig.as_str() == p.to_string_lossy()) {
            continue;
        }
        if p.is_dir() {
            if SKIP.contains(&name.as_str()) {
                continue;
            }
            collect_docs(&p, ignored, out, depth - 1);
        } else if is_doc_file(&name) {
            out.push(p);
        }
    }
}

/// Empaqueta en un .zip solo los archivos de documentación del proyecto.
#[tauri::command]
fn export_zip(project_dir: String, dest: String, ignored: Vec<String>) -> Result<usize, String> {
    let base = Path::new(&project_dir);
    let mut files: Vec<PathBuf> = Vec::new();
    collect_docs(base, &ignored, &mut files, 10);
    let file = fs::File::create(&dest).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default();
    for f in &files {
        let rel = f.strip_prefix(base).unwrap_or(f).to_string_lossy().to_string();
        let data = fs::read(f).map_err(|e| e.to_string())?;
        zip.start_file(rel, opts).map_err(|e| e.to_string())?;
        zip.write_all(&data).map_err(|e| e.to_string())?;
    }
    zip.finish().map_err(|e| e.to_string())?;
    Ok(files.len())
}

/// Crea un proyecto/carpeta con los contratos (plantillas) dentro.
#[tauri::command]
fn create_project(root: String, name: String) -> Result<String, String> {
    let dir = Path::new(&root).join(&name);
    if dir.exists() {
        return Err("Ya existe una carpeta con ese nombre".to_string());
    }
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let templates: Vec<(&str, String)> = vec![
        ("README.md", format!("# {}\n> [Una frase]\n\n## Qué es\n\n## Estado\n- [ ] MVP\n", name)),
        ("IDEAS.md", format!("# Ideas — {}\n\n- \n", name)),
    ];
    for (fname, content) in templates {
        fs::write(dir.join(fname), content).map_err(|e| e.to_string())?;
    }
    Ok(dir.to_string_lossy().to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            default_root,
            list_dir,
            read_file,
            write_file,
            create_dir,
            read_image_data_uri,
            copy_into,
            delete_path,
            find_docs,
            project_tree,
            export_zip,
            create_project
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
