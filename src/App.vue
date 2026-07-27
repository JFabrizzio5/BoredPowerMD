<script setup lang="ts">
import { ref, reactive, computed, onMounted, watch, nextTick } from "vue";
import { invoke, convertFileSrc } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { openUrl, openPath, revealItemInDir } from "@tauri-apps/plugin-opener";
import { marked } from "marked";
import markedKatex from "marked-katex-extension";
import "katex/dist/katex.min.css";
import mermaid from "mermaid";
import TreeNode from "./TreeNode.vue";
import Icon from "./Icon.vue";

marked.use(markedKatex({ throwOnError: false, nonStandard: true }));

type Entry = { name: string; path: string; is_dir: boolean; is_image: boolean; has_docs: boolean };
type Found = { file: string; path: string; rel: string };
type Category = { id: string; label: string; icon: string; file: string; patterns: string[] };
type Cat = Category & { instances: Found[] };

const DOC_BODIES: Record<string, string> = {
  "README.md": "# [Name]\n> [One line]\n\n## What it is\n\n## Status\n- [ ] MVP\n",
  "ARCHITECTURE.md": "# Architecture\n\n## Diagram\n![diagram](diagram.png)\n\n## The \"hard part\"\n\n## Protocols\n[REST · gRPC · MQTT · event bus]\n",
  "SECURITY.md": "# Security policy\n\n## Threat model\n| Threat | Mitigation |\n|---|---|\n|  |  |\n\n## Secrets\n- Never in git\n",
  "PRODUCT.md": "# Product\n\n## Problem & user\n\n## Market\n- Differentiator:\n\n## Core loop\n1.\n",
  "_CONTRATO.md": "# 📜 Contract\n\n## Rigor\n- [ ] Tests 70%+\n- [ ] Green CI\n- [ ] SECURITY.md\n\n## Business\n- [ ] Sales week\n\n## Finishing\n- [ ] Deployed\n- [ ] Video\n",
  "IDEAS.md": "# Ideas\n\n- \n",
  "CLAUDE.md": "# CLAUDE.md\n> Context for AI agents (Claude Code, etc.)\n\n## What this project is\n\n## Stack\n\n## Commands\n```bash\n```\n\n## Conventions\n",
};
const DEFAULT_CATEGORIES: Category[] = [
  { id: "readme", label: "README", icon: "book", file: "README.md", patterns: ["README.md"] },
  { id: "arch", label: "Architecture", icon: "layers", file: "ARCHITECTURE.md", patterns: ["ARCHITECTURE.md"] },
  { id: "security", label: "Security", icon: "shield", file: "SECURITY.md", patterns: ["SECURITY.md"] },
  { id: "product", label: "Product", icon: "package", file: "PRODUCT.md", patterns: ["PRODUCT.md"] },
  { id: "contract", label: "Contract", icon: "scroll", file: "_CONTRATO.md", patterns: ["_CONTRATO.md"] },
  { id: "ideas", label: "Ideas", icon: "lightbulb", file: "IDEAS.md", patterns: ["IDEAS.md"] },
  { id: "ai", label: "AI Context", icon: "sparkles", file: "CLAUDE.md", patterns: ["CLAUDE.md"] },
];
const ICON_CHOICES = ["book", "layers", "shield", "package", "file-text", "scroll", "lightbulb", "sparkles", "folder", "star", "code", "flag", "rocket", "settings"];

const root = ref(localStorage.getItem("root") || "");
const openMap = reactive<Record<string, boolean>>({});
const childMap = reactive<Record<string, Entry[]>>({});
const docsOnly = ref(true);
const search = ref("");
const error = ref("");

const currentDir = ref("");
const current = ref<Entry | null>(null);
const content = ref("");
const rendered = ref("");
const imageSrc = ref("");
const pdfSrc = ref("");
const mode = ref<"view" | "edit">("view");
const dirty = ref(false);
const saved = ref(false);

const creating = ref<null | "folder" | "note">(null);
const newName = ref("");
const pendingDelete = ref<Entry | null>(null);

const DOC_EXTENSIONS = ["*.md", "*.markdown", "*.txt", "*.pdf", "*.html", "*.htm"];

// ---- theme (light / dark, manual + persisted) ----
const dark = ref((localStorage.getItem("theme") || (window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light")) === "dark");
function applyTheme() { document.documentElement.classList.toggle("dark", dark.value); }
function toggleTheme() { dark.value = !dark.value; localStorage.setItem("theme", dark.value ? "dark" : "light"); applyTheme(); }
applyTheme();

// ---- accent color (customizable, persisted) ----
const ACCENT_PRESETS = ["#bd8b52", "#4a4ad8", "#0f9d8f", "#c0567a", "#5a8f3f", "#d4772b", "#7a5cd6"];
const accent = ref(localStorage.getItem("accent") || "");
function applyAccent() {
  const el = document.documentElement;
  if (accent.value) {
    el.style.setProperty("--accent", accent.value);
    el.style.setProperty("--accent-soft", accent.value + "22");
  } else {
    el.style.removeProperty("--accent");
    el.style.removeProperty("--accent-soft");
  }
}
function setAccent(hex: string) { accent.value = hex; localStorage.setItem("accent", hex); applyAccent(); }
function resetAccent() { accent.value = ""; localStorage.removeItem("accent"); applyAccent(); }
applyAccent();

// ---- AI doc generator (copy-paste to any AI, no API key) ----
const aiModal = ref(false);
const aiPrompt = ref("");
const aiResponse = ref("");
const aiStatus = ref("");
function buildPrompt(name: string, tree: string): string {
  return `Eres un experto en documentación técnica. Analiza este proyecto y genera su documentación.

PROYECTO: ${name}

ESTRUCTURA DE ARCHIVOS:
${tree}

Genera:
1. Un README.md completo (qué es, stack, cómo correr, estructura).
2. Un ARCHITECTURE.md (componentes principales, flujo de datos, decisiones clave).
3. Sugiere de 3 a 6 categorías/topics de documentación útiles para ESTE proyecto.

Responde EXACTAMENTE con este formato (respeta los delimitadores, sin texto extra):

===FILE: README.md===
(contenido markdown)
===END===

===FILE: ARCHITECTURE.md===
(contenido markdown)
===END===

===TOPICS===
- NombreCategoria | archivo.md | *.ext (el patrón extra es opcional)
===END===`;
}
async function openAi() {
  if (!projectDir.value) return;
  aiStatus.value = ""; aiResponse.value = "";
  const tree = await invoke<string>("project_tree", { path: projectDir.value });
  aiPrompt.value = buildPrompt(projectName.value, tree);
  aiModal.value = true;
}
async function copyPrompt() {
  try { await navigator.clipboard.writeText(aiPrompt.value); aiStatus.value = "¡Copiado! Pégalo en tu IA."; }
  catch { aiStatus.value = "Selecciónalo y cópialo manualmente."; }
}
function stripFence(s: string): string {
  return s.replace(/^\s*```[a-z]*\n?/i, "").replace(/\n?```\s*$/i, "").trim();
}
async function applyAi() {
  const text = aiResponse.value;
  if (!text.trim()) { aiStatus.value = "Pega primero la respuesta de la IA."; return; }
  let created = 0;
  const fileRe = /===FILE:\s*(.+?)===\s*\n([\s\S]*?)\n===END===/g;
  let m: RegExpExecArray | null;
  while ((m = fileRe.exec(text))) {
    const fname = m[1].trim().replace(/[/\\]/g, "-");
    const body = stripFence(m[2]);
    try { await invoke("write_file", { path: `${projectDir.value}/${fname}`, content: body + "\n" }); created++; } catch { /* */ }
  }
  const tm = /===TOPICS===\s*\n([\s\S]*?)\n===END===/.exec(text);
  if (tm) {
    for (const raw of tm[1].split("\n")) {
      const line = raw.replace(/^[-*]\s*/, "").trim();
      if (!line) continue;
      const parts = line.split("|").map((s) => s.trim());
      const label = parts[0];
      if (!label || categories.value.some((c) => c.label.toLowerCase() === label.toLowerCase())) continue;
      const file = parts[1] || label.toUpperCase().replace(/[^A-Z0-9]+/g, "_").replace(/^_|_$/g, "") + ".md";
      const patterns = [file];
      if (parts[2]) patterns.push(parts[2]);
      categories.value.push({ id: "c" + Date.now() + Math.floor(Math.random() * 1e4), label, icon: "file-text", file, patterns });
    }
    saveCategories();
  }
  openMap[projectDir.value] = true;
  await loadDir(projectDir.value);
  await refreshFound();
  aiStatus.value = created ? `Listo: ${created} archivo(s) creado(s).` : "No encontré bloques ===FILE===. Revisa el formato.";
  if (created) setTimeout(() => { aiModal.value = false; }, 1400);
}

// ---- toast ----
const toast = ref("");
function showToast(msg: string) { toast.value = msg; setTimeout(() => { if (toast.value === msg) toast.value = ""; }, 2600); }

// ---- ignorar archivos (click derecho) ----
const ignored = ref<string[]>(JSON.parse(localStorage.getItem("ignored") || "[]"));
function isIgnored(path: string) { return ignored.value.includes(path); }
function toggleIgnore(path: string) {
  ignored.value = isIgnored(path) ? ignored.value.filter((p) => p !== path) : [...ignored.value, path];
  localStorage.setItem("ignored", JSON.stringify(ignored.value));
}
const ctx = ref<{ x: number; y: number; node: Entry } | null>(null);
function showCtx(node: Entry, e: MouseEvent) { ctx.value = { x: Math.min(e.clientX, window.innerWidth - 160), y: e.clientY, node }; }
function closeCtx() { ctx.value = null; }

// ---- exportar ZIP (solo docs) ----
async function exportZip() {
  if (!projectDir.value) return;
  try {
    const dest = await saveDialog({ defaultPath: `${projectName.value}-docs.zip`, filters: [{ name: "ZIP", extensions: ["zip"] }] });
    if (typeof dest !== "string") { showToast("Exportación cancelada."); return; }
    showToast("Creando ZIP…");
    const n = await invoke<number>("export_zip", { projectDir: projectDir.value, dest, ignored: ignored.value });
    showToast(`ZIP guardado: ${n} archivo(s).`);
    try { await revealPath(dest); } catch { /* */ }
  } catch (e) { error.value = String(e); showToast("Error al crear ZIP: " + String(e)); }
}

// muestra el archivo en Finder para que el usuario lo vea
async function revealPath(p: string) {
  try { await revealItemInDir(p); } catch { try { await openPath(p.replace(/[/\\][^/\\]+$/, "")); } catch { /* */ } }
}

// abre el HTML actual en el navegador del sistema (ahí los links/redirects sí funcionan)
async function openHtmlExternal() {
  if (!current.value) return;
  try { await openPath(current.value.path); showToast("Abierto en el navegador."); }
  catch (e) { error.value = String(e); showToast("No se pudo abrir: " + String(e)); }
}

// CSS de impresión para el HTML exportado
const PDF_CSS = `
  :root{color:#111;background:#fff;}
  html,body{overflow:visible!important;height:auto!important;}
  body.pdfbody{font-family:-apple-system,system-ui,sans-serif;margin:0;color:#111;background:#fff;max-width:900px;margin:0 auto;}
  .pdf-cover{min-height:60vh;display:flex;flex-direction:column;justify-content:center;padding:0 14mm;page-break-after:always;}
  .pdf-cover .pk{color:#bd8b52;font-weight:600;text-transform:uppercase;letter-spacing:.12em;font-size:12px;}
  .pdf-cover h1{font-size:34px;margin:12px 0;color:#111;border:none;}
  .pdf-cover .pp{color:#999;font-size:13px;font-family:monospace;}
  .pdf-content{padding:8mm 14mm 14mm;font-size:14px;line-height:1.65;page-break-after:always;}
  .pdf-content h1{font-size:22px;} .pdf-content h2{font-size:18px;border-bottom:1px solid #eee;padding-bottom:4px;}
  .pdf-content img,.pdf-content svg,.mermaid-diagram svg{max-width:100%;height:auto;}
  .pdf-content pre{background:#f5f3ee;padding:12px 14px;border-radius:8px;overflow-x:auto;} .pdf-content code{background:#f5f3ee;padding:1px 5px;border-radius:4px;}
  .pdf-content table{border-collapse:collapse;} .pdf-content th,.pdf-content td{border:1px solid #ddd;padding:5px 10px;}
  .pdf-content blockquote{border-left:3px solid #bd8b52;margin:10px 0;padding:2px 12px;color:#666;}
  .codecopy{display:none!important;} .mermaid-diagram{border:1px solid #eee;border-radius:8px;padding:12px;text-align:center;}
  @media print{@page{margin:12mm;} .pdfbody{max-width:none;}}
`;

// ---- exportar PDF: genera HTML autocontenido, lo guardas donde quieras y se abre para imprimir ----
async function exportPdf() {
  if (!projectDir.value) return;
  const mds = foundDocs.value.filter((f) => /\.md$/i.test(f.file) && !isIgnored(f.path));
  if (!mds.length) { showToast("No hay .md para exportar."); return; }
  const dest = await saveDialog({ defaultPath: `${projectName.value}-docs.html`, filters: [{ name: "Documento imprimible (HTML → PDF)", extensions: ["html"] }] });
  if (typeof dest !== "string") { showToast("Exportación cancelada."); return; }
  showToast("Generando documento…");
  let body = "";
  for (const f of mds) {
    let md = "";
    try { md = await invoke<string>("read_file", { path: f.path }); } catch { continue; }
    const inner = await mdToHtml(md, parentOf(f.path), false);
    const title = (md.match(/^#\s+(.+)$/m)?.[1] || f.file).trim();
    const loc = f.rel ? `${f.rel} / ${f.file}` : f.file;
    body += `<section class="pdf-cover"><div class="pk">${projectName.value}</div><h1>${title}</h1><div class="pp">${loc}</div></section><section class="pdf-content">${inner}</section>`;
  }
  // inline de todo el CSS cargado (incluye KaTeX) para que el HTML sea autocontenido
  const inlineCss = Array.from(document.styleSheets).map((s) => {
    try { return Array.from(s.cssRules).map((r) => r.cssText).join("\n"); } catch { return ""; }
  }).join("\n");
  const full = `<!doctype html><html lang="es"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>${projectName.value} — documentación</title>
<style>${inlineCss}</style>
<style>${PDF_CSS}</style>
</head><body class="pdfbody">${body}
<script>window.addEventListener('load',function(){setTimeout(function(){try{window.print();}catch(e){}} ,400);});<\/script>
</body></html>`;
  try {
    await invoke("write_file", { path: dest, content: full });
    await openPath(dest);
    showToast("Guardado y abierto en el navegador. Usa Cmd+P → Guardar como PDF.");
  } catch (e) { error.value = String(e); showToast("Error al exportar: " + String(e)); }
}

// ---- categories (editable, persisted) ----
function loadCategories(): Category[] {
  try { const s = localStorage.getItem("categories"); if (s) return JSON.parse(s); } catch { /* */ }
  return JSON.parse(JSON.stringify(DEFAULT_CATEGORIES));
}
const categories = ref<Category[]>(loadCategories());
const showConfig = ref(false);
const emojiInput = reactive<Record<string, string>>({});
const newPattern = reactive<Record<string, string>>({});
const newCatName = ref("");

function saveCategories() { localStorage.setItem("categories", JSON.stringify(categories.value)); refreshFound(); }
function addCategory() {
  const name = newCatName.value.trim();
  if (!name) return;
  const file = name.toUpperCase().replace(/[^A-Z0-9]+/g, "_").replace(/^_|_$/g, "") + ".md";
  categories.value.push({ id: "c" + Date.now(), label: name, icon: "file-text", file, patterns: [file] });
  newCatName.value = "";
  saveCategories();
}
function removeCategory(i: number) { categories.value.splice(i, 1); saveCategories(); }
function setIcon(cat: Category, icon: string) { cat.icon = icon; saveCategories(); }
function setEmoji(cat: Category) {
  const v = (emojiInput[cat.id] || "").trim();
  if (v) { cat.icon = v; emojiInput[cat.id] = ""; saveCategories(); }
}
function addPattern(cat: Category) {
  const v = (newPattern[cat.id] || "").trim();
  if (!v) return;
  if (!cat.patterns.includes(v)) cat.patterns.push(v);
  newPattern[cat.id] = "";
  saveCategories();
}
function removePattern(cat: Category, i: number) { cat.patterns.splice(i, 1); saveCategories(); }

function matchesPattern(name: string, pat: string): boolean {
  const n = name.toLowerCase(); const p = pat.toLowerCase();
  if (p.startsWith("*.")) return n.endsWith("." + p.slice(2));
  if (p.startsWith(".")) return n.endsWith(p);
  return n === p;
}
function categoryOf(name: string): string | null {
  for (const c of categories.value) if (c.patterns.some((pat) => matchesPattern(name, pat))) return c.id;
  return null;
}

function isDoc(e: Entry) {
  if (e.is_dir) return e.has_docs;
  return e.is_image || /\.(md|markdown|txt|pdf|html?)$/i.test(e.name);
}
function parentOf(p: string) { return p.slice(0, p.lastIndexOf("/")); }

const topLevel = computed(() =>
  (childMap[root.value] || [])
    .filter((e) => (docsOnly.value ? isDoc(e) : true))
    .filter((e) => e.name.toLowerCase().includes(search.value.toLowerCase()))
);
const dirLabel = computed(() => currentDir.value.split("/").pop() || "raíz");
const isMd = computed(() => !!current.value && current.value.name.toLowerCase().endsWith(".md"));
const isPdf = computed(() => !!current.value && current.value.name.toLowerCase().endsWith(".pdf"));
const isHtml = computed(() => !!current.value && /\.html?$/i.test(current.value.name));
const htmlSrc = computed(() => (isHtml.value && current.value ? convertFileSrc(current.value.path) : ""));
// documento HTML con <base> inyectada para que resuelva sus recursos (imágenes, css, js)
const htmlDoc = computed(() => {
  if (!isHtml.value || !current.value) return "";
  const base = `<base href="${htmlSrc.value}">`;
  const c = content.value || "";
  if (/<head[^>]*>/i.test(c)) return c.replace(/<head([^>]*)>/i, `<head$1>${base}`);
  if (/<html[^>]*>/i.test(c)) return c.replace(/<html([^>]*)>/i, `<html$1><head>${base}</head>`);
  return `<!doctype html><html><head><meta charset="utf-8">${base}</head><body>${c}</body></html>`;
});

// intercepta los clicks de links dentro del HTML embebido y abre el destino en el navegador
function onHtmlFrameLoad(e: Event) {
  const frame = e.target as HTMLIFrameElement;
  let doc: Document | null = null;
  try { doc = frame.contentDocument; } catch { return; }
  if (!doc) return;
  doc.addEventListener("click", (ev) => {
    const a = (ev.target as HTMLElement)?.closest?.("a") as HTMLAnchorElement | null;
    if (!a) return;
    const raw = a.getAttribute("href") || "";
    if (!raw || raw.startsWith("#") || raw.startsWith("javascript:")) return; // ancla interna / js: deja el comportamiento normal
    ev.preventDefault();
    if (/^(https?:|mailto:|tel:)/i.test(raw)) { openUrl(raw).catch(() => {}); return; }
    // link a un archivo local (relativo, secundario o absoluto) → abrir en el navegador
    try {
      const bp = current.value ? current.value.path : "";
      const abs = raw.startsWith("file://")
        ? decodeURIComponent(new URL(raw).pathname)
        : decodeURIComponent(new URL(raw, "file://" + encodeURI(bp)).pathname);
      openPath(abs).then(() => showToast("Abierto en el navegador: " + abs.split("/").pop())).catch((err) => showToast("No se pudo abrir: " + String(err)));
    } catch (err) { showToast("Ruta no válida: " + String(err)); }
  }, true);
}
const crumbs = computed(() => {
  const base = current.value ? current.value.path : currentDir.value;
  if (!base || !base.startsWith(root.value)) return [];
  return base.slice(root.value.length).split("/").filter(Boolean);
});
const projectName = computed(() => crumbs.value[0] || "");
const projectDir = computed(() => (projectName.value ? `${root.value}/${projectName.value}` : ""));

const foundDocs = ref<Found[]>([]);
async function refreshFound() {
  if (!projectDir.value) { foundDocs.value = []; return; }
  try { foundDocs.value = await invoke<Found[]>("find_docs", { projectDir: projectDir.value, names: DOC_EXTENSIONS }); }
  catch { foundDocs.value = []; }
}
const landingCats = computed<Cat[]>(() => {
  const cats = categories.value.map((c) => ({ ...c, instances: foundDocs.value.filter((f) => categoryOf(f.file) === c.id) }));
  const extras = foundDocs.value.filter((f) => categoryOf(f.file) === null);
  if (extras.length) cats.push({ id: "__extras", label: "Extras", icon: "folder", file: "", patterns: [], instances: extras });
  return cats;
});
const drillCat = ref<string | null>(null);
const drillView = computed(() => landingCats.value.find((c) => c.id === drillCat.value) || null);
// filtro dentro de una categoría (buscador + formato) para no saturar el front
const drillQuery = ref("");
const drillFmt = ref("all");
watch(drillCat, () => { drillQuery.value = ""; drillFmt.value = "all"; });
function fmtOf(name: string) { const m = name.match(/\.([a-z0-9]+)$/i); return m ? m[1].toLowerCase() : "—"; }
const drillFormats = computed(() => {
  if (!drillView.value) return [] as string[];
  const s = new Set<string>();
  drillView.value.instances.forEach((f) => s.add(fmtOf(f.file)));
  return Array.from(s).sort();
});
const drillInstances = computed(() => {
  if (!drillView.value) return [] as Found[];
  const q = drillQuery.value.trim().toLowerCase();
  return drillView.value.instances.filter((f) => {
    if (drillFmt.value !== "all" && fmtOf(f.file) !== drillFmt.value) return false;
    if (q && !`${f.file} ${f.rel || ""}`.toLowerCase().includes(q)) return false;
    return true;
  });
});

watch(content, () => { dirty.value = true; saved.value = false; });
watch(projectDir, async (p) => { drillCat.value = null; if (p && !childMap[p]) await loadDir(p); await refreshFound(); });

async function loadDir(path: string) {
  try { childMap[path] = await invoke<Entry[]>("list_dir", { path }); error.value = ""; }
  catch (e) { error.value = String(e); }
}
async function init() {
  if (!root.value) { root.value = await invoke<string>("default_root"); localStorage.setItem("root", root.value); }
  currentDir.value = root.value;
  openMap[root.value] = true;
  await loadDir(root.value);
}
function applyRoot(path: string) {
  root.value = path;
  localStorage.setItem("root", path);
  for (const k of Object.keys(openMap)) delete openMap[k];
  for (const k of Object.keys(childMap)) delete childMap[k];
  current.value = null;
  currentDir.value = path;
  openMap[path] = true;
  loadDir(path);
}
async function browsePath() {
  try {
    const sel = await open({ directory: true, multiple: false, defaultPath: root.value || undefined });
    if (typeof sel === "string") applyRoot(sel);
  } catch (e) { error.value = String(e); }
}
async function toggle(node: Entry) {
  currentDir.value = node.path;
  current.value = null;
  imageSrc.value = ""; pdfSrc.value = ""; rendered.value = ""; content.value = "";
  openMap[node.path] = !openMap[node.path];
  if (openMap[node.path] && !childMap[node.path]) await loadDir(node.path);
}
async function onSelect(node: Entry) {
  currentDir.value = parentOf(node.path);
  await openFile(node);
}
async function openFile(f: Entry) {
  current.value = f;
  drillCat.value = null;
  imageSrc.value = ""; pdfSrc.value = ""; rendered.value = ""; content.value = "";
  const lower = f.name.toLowerCase();
  if (f.is_image) {
    imageSrc.value = await invoke<string>("read_image_data_uri", { path: f.path });
  } else if (lower.endsWith(".pdf")) {
    pdfSrc.value = await invoke<string>("read_image_data_uri", { path: f.path });
  } else {
    try { content.value = await invoke<string>("read_file", { path: f.path }); } catch { content.value = ""; }
    if (lower.endsWith(".md")) { mode.value = "view"; await renderMd(); }
    else if (/\.html?$/.test(lower)) { mode.value = "view"; }
    else { mode.value = "edit"; }
  }
  dirty.value = false; saved.value = false;
}
async function mdToHtml(md: string, dir: string, interactive = true): Promise<string> {
  // codifica espacios en URLs de imágenes locales para que markdown las parsee
  const fixed = md.replace(/(!\[[^\]]*\]\()([^)]+)(\))/g, (m, pre, url, post) => {
    const u = url.trim();
    if (/^(https?:|data:|<)/i.test(u)) return m;
    return pre + u.replace(/ /g, "%20") + post;
  });
  const doc = new DOMParser().parseFromString(String(await marked.parse(fixed)), "text/html");
  for (const img of Array.from(doc.querySelectorAll("img"))) {
    const src = img.getAttribute("src") || "";
    if (!/^(https?:|data:)/i.test(src)) {
      const raw = src.startsWith("/") ? src : `${dir}/${src}`;
      let abs = raw;
      try { abs = decodeURIComponent(raw); } catch { /* */ }
      try {
        img.setAttribute("src", await invoke<string>("read_image_data_uri", { path: abs }));
      } catch {
        const ph = doc.createElement("span"); ph.className = "imgmissing";
        ph.textContent = "Imagen no encontrada: " + decodeURIComponent(src);
        img.replaceWith(ph);
      }
    }
  }
  const mmd = Array.from(doc.querySelectorAll("code.language-mermaid, code.lang-mermaid"));
  if (mmd.length) {
    mermaid.initialize({ startOnLoad: false, securityLevel: "loose", theme: dark.value ? "dark" : "default" });
    for (let i = 0; i < mmd.length; i++) {
      const src = mmd[i].textContent || "";
      const host = mmd[i].closest("pre") || mmd[i];
      try {
        const { svg } = await mermaid.render("mmd-" + Date.now() + "-" + i, src);
        const wrap = doc.createElement("div"); wrap.className = "mermaid-diagram"; wrap.innerHTML = svg;
        host.replaceWith(wrap);
      } catch (e) {
        const err = doc.createElement("div"); err.className = "imgmissing";
        err.textContent = "Diagrama Mermaid inválido: " + String(e).slice(0, 120);
        host.replaceWith(err);
      }
    }
  }
  if (interactive) {
    for (const cb of Array.from(doc.querySelectorAll('input[type="checkbox"]'))) cb.removeAttribute("disabled");
    for (const pre of Array.from(doc.querySelectorAll("pre"))) {
      const btn = doc.createElement("button"); btn.className = "codecopy"; btn.type = "button"; btn.textContent = "copiar";
      pre.insertBefore(btn, pre.firstChild);
    }
  }
  return doc.body.innerHTML;
}
async function renderMd() {
  if (!current.value) return;
  rendered.value = await mdToHtml(content.value, parentOf(current.value.path), true);
}

async function onMdClick(e: MouseEvent) {
  const t = e.target as HTMLElement;
  if (!t) return;
  // enlaces → abrir en el navegador del sistema
  const a = t.closest("a");
  if (a && a.getAttribute("href")) {
    e.preventDefault();
    const href = a.getAttribute("href")!;
    if (/^https?:/i.test(href)) { try { await openUrl(href); } catch { /* */ } }
    return;
  }
  // copiar bloque de código
  if (t.classList.contains("codecopy")) {
    const code = t.closest("pre")?.querySelector("code")?.textContent || "";
    try { await navigator.clipboard.writeText(code); t.textContent = "¡copiado!"; setTimeout(() => (t.textContent = "copiar"), 1200); } catch { /* */ }
    return;
  }
  // casillas de tarea → toggle + guardar
  if (t.tagName === "INPUT" && (t as HTMLInputElement).type === "checkbox") {
    const body = e.currentTarget as HTMLElement;
    const idx = Array.from(body.querySelectorAll('input[type="checkbox"]')).indexOf(t);
    if (idx < 0) return;
    let count = 0;
    content.value = content.value.replace(/^(\s*[-*+]\s+)\[([ xX])\]/gm, (match, p1, p2) => {
      const cur = count; count++;
      return cur === idx ? `${p1}[${p2 === " " ? "x" : " "}]` : match;
    });
    await save();
  }
}
const editor = ref<HTMLTextAreaElement | null>(null);
function insertAtCursor(before: string, after = "", placeholder = "") {
  const ta = editor.value;
  if (!ta) { content.value += before + placeholder + after; return; }
  const start = ta.selectionStart, end = ta.selectionEnd;
  const sel = content.value.slice(start, end) || placeholder;
  content.value = content.value.slice(0, start) + before + sel + after + content.value.slice(end);
  nextTick(() => {
    ta.focus();
    ta.setSelectionRange(start + before.length, start + before.length + sel.length);
  });
}
function insertTable() { insertAtCursor("\n| Col A | Col B |\n|---|---|\n| a | b |\n"); }
async function addImage() {
  if (!current.value) return;
  try {
    const sel = await open({ multiple: false, filters: [{ name: "Imágenes", extensions: ["png", "jpg", "jpeg", "gif", "svg", "webp", "bmp"] }] });
    if (typeof sel !== "string") return;
    const dir = parentOf(current.value.path);
    const destPath = await invoke<string>("copy_into", { src: sel, destDir: dir });
    const fname = destPath.split("/").pop() || "imagen";
    insertAtCursor(`![${fname}](${fname})`);
  } catch (e) { error.value = String(e); }
}

async function setMode(m: "view" | "edit") { if (m === "view" && isMd.value) await renderMd(); mode.value = m; }
async function save() {
  if (!current.value) return;
  try {
    await invoke("write_file", { path: current.value.path, content: content.value });
    dirty.value = false; saved.value = true;
    if (isMd.value) await renderMd();
  } catch (e) { error.value = String(e); }
}
async function refreshCurrentDir() { openMap[currentDir.value] = true; await loadDir(currentDir.value); }
function startCreate(kind: "folder" | "note") { creating.value = kind; newName.value = ""; }
async function confirmCreate() {
  const name = newName.value.trim();
  if (!name) { creating.value = null; return; }
  try {
    if (creating.value === "folder") {
      await invoke("create_dir", { path: `${currentDir.value}/${name}` });
    } else {
      const fname = name.toLowerCase().endsWith(".md") ? name : `${name}.md`;
      const path = `${currentDir.value}/${fname}`;
      await invoke("write_file", { path, content: `# ${name.replace(/\.md$/, "")}\n\n` });
      await openFile({ name: fname, path, is_dir: false, is_image: false, has_docs: false });
    }
    creating.value = null; newName.value = "";
    await refreshCurrentDir();
  } catch (e) { error.value = String(e); }
}
async function insertTemplate(file: string) {
  if (!file) return;
  const path = `${currentDir.value}/${file}`;
  try {
    await invoke("write_file", { path, content: DOC_BODIES[file] || `# ${file}\n\n` });
    await openFile({ name: file, path, is_dir: false, is_image: false, has_docs: false });
    await refreshCurrentDir();
  } catch (e) { error.value = String(e); }
}
async function openInstance(inst: Found) {
  await openFile({ name: inst.file, path: inst.path, is_dir: false, is_image: false, has_docs: false });
}
async function clickCat(cat: Cat) {
  if (cat.instances.length === 0) {
    if (!cat.file) return;
    const path = `${projectDir.value}/${cat.file}`;
    await invoke("write_file", { path, content: DOC_BODIES[cat.file] || `# ${cat.label}\n\n` });
    openMap[projectDir.value] = true;
    await loadDir(projectDir.value);
    await refreshFound();
    await openFile({ name: cat.file, path, is_dir: false, is_image: false, has_docs: false });
  } else if (cat.instances.length === 1) {
    await openInstance(cat.instances[0]);
  } else {
    drillCat.value = cat.id;
  }
}
function askDelete(node: Entry) { pendingDelete.value = node; }
async function doDelete() {
  const n = pendingDelete.value;
  if (!n) return;
  try {
    await invoke("delete_path", { path: n.path });
    if (current.value && (current.value.path === n.path || current.value.path.startsWith(n.path + "/"))) current.value = null;
    const parent = parentOf(n.path);
    await loadDir(parent);
    if (currentDir.value === n.path || currentDir.value.startsWith(n.path + "/")) currentDir.value = parent;
  } catch (e) { error.value = String(e); }
  pendingDelete.value = null;
}
function onKey(e: KeyboardEvent) {
  if ((e.metaKey || e.ctrlKey) && e.key === "s") { e.preventDefault(); save(); }
}
onMounted(async () => {
  await init();
  try {
    await getCurrentWebview().onDragDropEvent(async (event: any) => {
      if (event.payload?.type === "drop" && Array.isArray(event.payload.paths)) {
        for (const p of event.payload.paths) { try { await invoke("copy_into", { src: p, destDir: currentDir.value }); } catch { /* */ } }
        await refreshCurrentDir();
      }
    });
  } catch { /* */ }
});
</script>

<template>
  <div class="app" @keydown="onKey">
    <aside class="sidebar">
      <div class="brand">
        <span class="dot"></span><h1>BoredPowerMD</h1>
        <div class="brandbtns">
          <button class="themebtn" @click="showConfig = true" title="Configuración (colores y categorías)"><Icon name="settings" :size="15" /></button>
          <button class="themebtn" @click="toggleTheme" title="Cambiar tema"><Icon :name="dark ? 'sun' : 'moon'" :size="15" /></button>
        </div>
      </div>

      <div class="rootbar">
        <span class="rootpath" :title="root">{{ root || "sin carpeta" }}</span>
        <button class="mini ghost" title="Elegir carpeta" @click="browsePath"><Icon name="folder" :size="13" /> cambiar</button>
      </div>

      <input class="search" v-model="search" placeholder="Buscar…" spellcheck="false" />

      <div class="toolbar">
        <button class="tbtn" @click="startCreate('folder')">＋ Carpeta</button>
        <button class="tbtn" @click="startCreate('note')">＋ Nota</button>
        <select class="tbtn sel" @change="insertTemplate(($event.target as HTMLSelectElement).value); ($event.target as HTMLSelectElement).value = ''">
          <option value="">Template…</option>
          <option v-for="c in categories" :key="c.file" :value="c.file">{{ c.label }}</option>
        </select>
      </div>

      <div v-if="creating" class="createrow">
        <input class="grow" v-model="newName" autofocus spellcheck="false"
          :placeholder="creating === 'folder' ? 'nombre-categoría' : 'nombre-nota'"
          @keydown.enter="confirmCreate" @keydown.esc="creating = null" />
        <button class="mini" @click="confirmCreate">crear</button>
      </div>

      <div class="filterbar">
        <button class="toggle" :class="{ on: docsOnly }" @click="docsOnly = !docsOnly">{{ docsOnly ? "Solo docs" : "Todo" }}</button>
        <span class="ctx">en <b>{{ dirLabel }}</b></span>
      </div>

      <div class="tree">
        <TreeNode v-for="e in topLevel" :key="e.path" :node="e" :depth="0" :open-map="openMap" :child-map="childMap"
          :active-path="current?.path || ''" :docs-only="docsOnly" :ignored="ignored"
          @toggle="toggle" @select="onSelect" @del="askDelete" @ctx="(p) => showCtx(p.node, p.e)" />
        <p v-if="!topLevel.length" class="empty">Vacío.</p>
      </div>
      <p v-if="error" class="err">{{ error }}</p>
    </aside>

    <main class="main">
      <div class="cfgoverlay" v-if="showConfig" @click.self="showConfig = false">
        <div class="cfgpanel">
          <div class="cfghead"><h3>Configuración</h3><button class="mini" @click="showConfig = false">cerrar</button></div>

          <div class="cfgcolors">
            <div class="cfglabel">Color de acento</div>
            <div class="swatches">
              <button v-for="col in ACCENT_PRESETS" :key="col" class="swatch" :class="{ on: accent === col }" :style="{ background: col }" @click="setAccent(col)"></button>
              <input type="color" class="colorpick" :value="accent || '#bd8b52'" @input="setAccent(($event.target as HTMLInputElement).value)" title="Personalizado" />
              <button class="cfgreset" @click="resetAccent">reset</button>
            </div>
          </div>

          <p class="cfghint">Cambia el icono, el nombre, o agrega archivos/extensiones (ej. <code>*.excalidraw</code>) que cuentan para cada categoría.</p>
          <div v-for="(cat, i) in categories" :key="cat.id" class="cfgrow">
            <div class="cfgtop">
              <span class="cfgcur"><Icon :name="cat.icon" :size="18" /></span>
              <input class="cfgname" v-model="cat.label" spellcheck="false" @change="saveCategories" />
              <button class="cfgdel" @click="removeCategory(i)">eliminar</button>
            </div>
            <div class="cfgicons">
              <button v-for="ic in ICON_CHOICES" :key="ic" class="cfgicon" :class="{ on: cat.icon === ic }" @click="setIcon(cat, ic)"><Icon :name="ic" :size="15" /></button>
              <input class="cfgemoji" v-model="emojiInput[cat.id]" placeholder="emoji" @keydown.enter="setEmoji(cat)" />
            </div>
            <div class="cfgchips">
              <span v-for="(p, j) in cat.patterns" :key="j" class="cfgchip">{{ p }}<span class="cx" @click="removePattern(cat, j)">×</span></span>
              <input class="cfginput" v-model="newPattern[cat.id]" placeholder="+ nombre o *.ext" spellcheck="false" @keydown.enter="addPattern(cat)" />
            </div>
          </div>
          <div class="cfgadd">
            <input v-model="newCatName" placeholder="Nueva categoría…" spellcheck="false" @keydown.enter="addCategory" />
            <button class="mini" @click="addCategory">＋ Agregar</button>
          </div>
        </div>
      </div>

      <div class="cfgoverlay" v-if="aiModal" @click.self="aiModal = false">
        <div class="cfgpanel aipanel">
          <div class="cfghead"><h3>Generar docs con IA</h3><button class="mini" @click="aiModal = false">cerrar</button></div>
          <p class="cfghint">1) Copia el prompt · 2) Pégalo en ChatGPT / Claude / cualquier IA · 3) Pega la respuesta aquí → crea los archivos y los topics automáticamente.</p>
          <div class="aistep">
            <div class="aisteplabel">1 · Prompt (ya incluye la estructura de tu proyecto)</div>
            <textarea class="aitext" readonly :value="aiPrompt"></textarea>
            <button class="mini genai" @click="copyPrompt"><Icon name="code" :size="13" /> Copiar prompt</button>
          </div>
          <div class="aistep">
            <div class="aisteplabel">2 · Pega aquí la respuesta de la IA</div>
            <textarea class="aitext" v-model="aiResponse" placeholder="Pega aquí lo que te devolvió la IA (con los bloques ===FILE===)…"></textarea>
            <div class="airow"><button class="mini genai" @click="applyAi"><Icon name="sparkles" :size="13" /> Crear archivos + topics</button><span v-if="aiStatus" class="ok">{{ aiStatus }}</span></div>
          </div>
        </div>
      </div>

      <div class="confirm" v-if="pendingDelete">
        <span>¿Eliminar <b>{{ pendingDelete.name }}</b><template v-if="pendingDelete.is_dir"> y todo su contenido</template>?</span>
        <button class="danger" @click="doDelete">Eliminar</button>
        <button class="mini" @click="pendingDelete = null">Cancelar</button>
      </div>

      <header class="head">
        <div class="crumb">
          <template v-if="crumbs.length">
            <span v-for="(c, i) in crumbs" :key="i" :class="{ proj: i === 0 }">{{ c }}<span v-if="i < crumbs.length - 1" class="sep"> › </span></span>
          </template>
          <span v-else class="dim">BoredPowerMD — elige una carpeta</span>
          <span v-if="dirty" class="dirty"> •</span>
        </div>
        <div class="actions" v-if="current && !current.is_image && !isPdf">
          <div class="seg" v-if="isMd || isHtml">
            <button :class="{ on: mode === 'view' }" @click="setMode('view')">Ver</button>
            <button :class="{ on: mode === 'edit' }" @click="setMode('edit')">Editar</button>
          </div>
          <button v-if="isHtml" class="mini" @click="openHtmlExternal" title="Abre el HTML en tu navegador (los links y redirecciones funcionan ahí)">Abrir en navegador ↗</button>
          <span v-if="saved" class="ok">Guardado ✓</span>
          <button class="save" @click="save">Guardar <kbd>⌘S</kbd></button>
        </div>
      </header>

      <div class="cats" v-if="projectDir && current">
        <span class="catl">{{ projectName }} ·</span>
        <button v-for="c in landingCats" :key="c.id" class="cat" :class="{ missing: c.instances.length === 0 }" @click="clickCat(c)">
          <Icon :name="c.icon" :size="13" /> {{ c.label }}<span v-if="c.instances.length === 0" class="plus">＋</span><span v-else-if="c.instances.length > 1" class="cnt"> ·{{ c.instances.length }}</span>
        </button>
      </div>

      <div class="body">
        <div class="landing drill" v-if="drillView">
          <button class="backbtn" @click="drillCat = null">← volver</button>
          <div class="lhead"><div><h1><Icon :name="drillView.icon" :size="24" /> {{ drillView.label }}</h1><p>{{ drillInstances.length }}<span v-if="drillInstances.length !== drillView.instances.length"> de {{ drillView.instances.length }}</span> archivos</p></div></div>
          <div class="drillfilter" v-if="drillView.instances.length > 5">
            <input class="dsearch" v-model="drillQuery" placeholder="Buscar por nombre o carpeta…" />
            <div class="fchips">
              <button class="fchip" :class="{ on: drillFmt === 'all' }" @click="drillFmt = 'all'">Todos</button>
              <button v-for="fmt in drillFormats" :key="fmt" class="fchip" :class="{ on: drillFmt === fmt }" @click="drillFmt = fmt">.{{ fmt }}</button>
            </div>
          </div>
          <div class="lgrid">
            <button v-for="(inst, i) in drillInstances" :key="i" class="lcard" @click="openInstance(inst)">
              <span class="licon"><Icon name="file-text" :size="24" /></span>
              <span class="llabel">{{ inst.file }}</span>
              <span class="lloc"><Icon name="folder" :size="12" /> {{ inst.rel || "root" }}</span>
              <span class="lstate">abrir →</span>
            </button>
          </div>
          <p v-if="!drillInstances.length" class="dim dempty">Sin resultados para ese filtro.</p>
        </div>
        <template v-else-if="current">
          <div v-if="current.is_image" class="imgwrap"><img class="img" :src="imageSrc" :alt="current.name" /></div>
          <iframe v-else-if="isPdf" class="pdf" :src="pdfSrc" :title="current.name"></iframe>
          <div v-else-if="isMd && mode === 'view'" class="md-body" v-html="rendered" @click="onMdClick"></div>
          <iframe v-else-if="isHtml && mode === 'view'" class="htmlview" :srcdoc="htmlDoc" @load="onHtmlFrameLoad" sandbox="allow-scripts allow-same-origin allow-popups allow-forms allow-modals" :title="current.name"></iframe>
          <div v-else class="editwrap">
            <div class="edtoolbar" v-if="isMd">
              <button @click="insertAtCursor('## ', '', 'Título')" title="Encabezado">H</button>
              <button class="tb" @click="insertAtCursor('**', '**', 'negrita')" title="Negrita">B</button>
              <button class="ti" @click="insertAtCursor('*', '*', 'cursiva')" title="Cursiva">I</button>
              <button @click="insertAtCursor('`', '`', 'code')" title="Código en línea"><Icon name="code" :size="13" /></button>
              <button @click="insertAtCursor('\n```\n', '\n```\n', 'código')" title="Bloque de código">```</button>
              <button @click="insertAtCursor('- ', '', 'item')" title="Lista">•</button>
              <button @click="insertAtCursor('- [ ] ', '', 'tarea')" title="Casilla">[ ]</button>
              <button @click="insertAtCursor('[', '](https://)', 'texto')" title="Enlace">link</button>
              <button @click="insertTable()" title="Tabla">tabla</button>
              <button @click="addImage" title="Insertar imagen desde tu gestor de archivos"><Icon name="image" :size="13" /></button>
              <button @click="insertAtCursor('$', '$', 'x^2')" title="LaTeX en línea">$x$</button>
              <button @click="insertAtCursor('\n$$\n', '\n$$\n', '\\int x\\,dx')" title="LaTeX en bloque">$$</button>
            </div>
            <textarea ref="editor" class="text" v-model="content" spellcheck="false" placeholder="Markdown…"></textarea>
          </div>
        </template>
        <div class="landing" v-else-if="projectDir">
          <div class="lhead">
            <div><h1>{{ projectName }}</h1><p>Navega la wiki</p></div>
            <div class="lheadbtns">
              <button class="mini genai" title="Generar documentación con IA" @click="openAi"><Icon name="sparkles" :size="14" /> Generar con IA</button>
              <button class="mini ghost" title="Exportar docs a PDF" @click="exportPdf"><Icon name="file-text" :size="14" /> PDF</button>
              <button class="mini ghost" title="Exportar docs a ZIP" @click="exportZip"><Icon name="package" :size="14" /> ZIP</button>
              <button class="mini ghost" title="Configurar categorías" @click="showConfig = true"><Icon name="settings" :size="14" /> categorías</button>
            </div>
          </div>
          <div class="lgrid">
            <button v-for="c in landingCats" :key="c.id" class="lcard" :class="{ missing: c.instances.length === 0 }" @click="clickCat(c)">
              <span class="licon"><Icon :name="c.icon" :size="24" /></span>
              <span class="llabel">{{ c.label }}</span>
              <span class="lstate">{{ c.instances.length === 0 ? "crear ＋" : c.instances.length === 1 ? "abrir →" : c.instances.length + " archivos →" }}</span>
            </button>
          </div>
        </div>
        <div class="welcome" v-else>
          <div class="dot big"></div>
          <h2>Tu wiki de proyectos</h2>
          <p>Abre una carpeta (izquierda) para ver sus <b>categorías</b>. Pasa el cursor sobre un item y usa <b>×</b> para eliminar.</p>
        </div>
      </div>
    </main>

    <div class="ctxback" v-if="ctx" @click="closeCtx" @contextmenu.prevent="closeCtx"></div>
    <div class="ctxmenu" v-if="ctx" :style="{ left: ctx.x + 'px', top: ctx.y + 'px' }">
      <button @click="toggleIgnore(ctx.node.path); closeCtx()">{{ isIgnored(ctx.node.path) ? "No ignorar" : "Ignorar" }}</button>
      <button class="del" @click="askDelete(ctx.node); closeCtx()">Eliminar</button>
    </div>
    <div class="toast" v-if="toast">{{ toast }}</div>
  </div>
</template>

<style>
:root {
  --bg: #faf9f6; --panel: #ffffff; --sidebar: #f4f2ec; --text: #1a160f;
  --muted: #6f6d63; --border: rgba(20,16,8,0.09); --accent: #bd8b52; --accent-soft: #f3e9db;
  --ok: #3f7d4e; --danger: #b4482f; --code: #f2efe8;
  font-family: -apple-system, "SF Pro Text", Inter, system-ui, sans-serif; color: var(--text);
}
:root.dark {
  --bg: #0e0e0d; --panel: #181816; --sidebar: #141312; --text: #f4f2ec;
  --muted: #928f86; --border: rgba(255,255,255,0.09); --accent: #d4a373; --accent-soft: #29241d;
  --ok: #5cbb7a; --danger: #e0836b; --code: #221f1b;
}
* { box-sizing: border-box; }
.sidebar, .main, body, .lcard, .cat, .save, .mini, .search, .toggle, .tbtn, input, textarea { transition: background-color .3s ease, color .3s ease, border-color .3s ease; }
@keyframes fadeUp { from { opacity: 0; transform: translateY(8px); } to { opacity: 1; transform: none; } }
@keyframes fade { from { opacity: 0; } to { opacity: 1; } }
html, body, #app { height: 100%; margin: 0; }
body { background: var(--bg); overflow: hidden; }
.app { display: grid; grid-template-columns: 280px 1fr; height: 100vh; }

.sidebar { background: var(--sidebar); border-right: 1px solid var(--border); display: flex; flex-direction: column; padding: 14px 12px; gap: 10px; min-height: 0; }
.brand { display: flex; align-items: center; gap: 9px; padding: 2px 4px; }
.brand h1 { font-size: 15px; font-weight: 600; margin: 0; }
.brandbtns { margin-left: auto; display: flex; gap: 6px; }
.themebtn { width: 28px; height: 28px; border-radius: 50%; border: 1px solid var(--border); background: var(--panel); color: var(--muted); display: flex; align-items: center; justify-content: center; cursor: pointer; transition: 0.18s; }
.themebtn:hover { color: var(--accent); border-color: var(--accent); transform: rotate(-15deg); }
.dot { width: 12px; height: 12px; border-radius: 50%; background: var(--accent); flex: none; }
.dot.big { width: 34px; height: 34px; margin-bottom: 6px; }

.rootbar { display: flex; align-items: center; gap: 6px; font-size: 11.5px; color: var(--muted); }
.rootpath { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; flex: 1; }
.grow { flex: 1; min-width: 0; }
.search { width: 100%; padding: 7px 10px; border-radius: 8px; border: 1px solid var(--border); background: var(--panel); color: var(--text); font-size: 13px; outline: none; }
.search:focus { border-color: var(--accent); }

.toolbar { display: flex; gap: 5px; }
.tbtn { flex: 1; border: 1px solid var(--border); background: var(--panel); color: var(--text); border-radius: 7px; padding: 6px 4px; font-size: 11.5px; cursor: pointer; font-family: inherit; }
.tbtn:hover { border-color: var(--accent); color: var(--accent); }
.sel { appearance: none; text-align: center; }
.createrow { display: flex; gap: 5px; }

.filterbar { display: flex; align-items: center; justify-content: space-between; gap: 8px; }
.toggle { border: 1px solid var(--border); background: var(--panel); color: var(--muted); border-radius: 7px; padding: 5px 10px; font-size: 11.5px; cursor: pointer; font-family: inherit; }
.toggle.on { background: var(--accent-soft); color: var(--accent); border-color: transparent; font-weight: 500; }
.ctx { font-size: 11px; color: var(--muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.ctx b { color: var(--text); font-weight: 500; }

.tree { flex: 1; overflow-y: auto; min-height: 0; margin: 0 -4px; padding: 0 4px; }

input { border: 1px solid var(--border); background: var(--panel); color: var(--text); border-radius: 7px; padding: 6px 9px; font-size: 12.5px; font-family: inherit; outline: none; }
input:focus { border-color: var(--accent); }
.mini { display: inline-flex; align-items: center; gap: 5px; border: 1px solid var(--border); background: var(--panel); color: var(--text); border-radius: 7px; padding: 6px 10px; font-size: 12px; cursor: pointer; font-weight: 500; }
.mini:hover { border-color: var(--accent); color: var(--accent); }
.mini.ghost { border-color: transparent; background: transparent; color: var(--muted); padding: 4px 6px; }
.mini.ghost:hover { color: var(--accent); }
.err { color: var(--danger); font-size: 11.5px; margin: 0; padding: 4px; }
.empty { color: var(--muted); font-size: 13px; padding: 6px 4px; }

.main { background: var(--bg); display: flex; flex-direction: column; min-height: 0; }
.confirm { display: flex; align-items: center; gap: 10px; padding: 10px 24px; background: var(--accent-soft); border-bottom: 1px solid var(--border); font-size: 13px; }
.confirm .danger { border: none; background: var(--danger); color: #fff; border-radius: 7px; padding: 5px 12px; font-size: 12.5px; font-weight: 500; cursor: pointer; }
.head { display: flex; align-items: center; justify-content: space-between; padding: 14px 24px; gap: 16px; border-bottom: 1px solid var(--border); min-height: 54px; }
.crumb { font-size: 13.5px; color: var(--muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.crumb .proj { color: var(--text); font-weight: 600; }
.crumb .dim { color: var(--muted); }
.dirty { color: var(--accent); }
.actions { display: flex; align-items: center; gap: 10px; flex: none; }
.seg { display: flex; background: var(--sidebar); border: 1px solid var(--border); border-radius: 8px; padding: 2px; }
.seg button { border: none; background: transparent; color: var(--muted); cursor: pointer; padding: 4px 12px; border-radius: 6px; font-size: 12px; font-weight: 500; }
.seg button.on { background: var(--panel); color: var(--text); }
.ok { color: var(--ok); font-size: 12px; }
.save { border: 1px solid var(--border); background: var(--panel); color: var(--text); border-radius: 7px; padding: 5px 12px; font-size: 12.5px; cursor: pointer; font-weight: 500; display: flex; align-items: center; gap: 6px; }
.save:hover { border-color: var(--accent); color: var(--accent); }
kbd { font-size: 10px; color: var(--muted); border: 1px solid var(--border); border-radius: 4px; padding: 0 4px; font-family: inherit; }

.cats { display: flex; align-items: center; gap: 6px; flex-wrap: wrap; padding: 10px 24px; border-bottom: 1px solid var(--border); }
.catl { font-size: 12px; color: var(--muted); font-weight: 500; }
.cat { display: inline-flex; align-items: center; gap: 5px; border: 1px solid var(--border); background: var(--panel); color: var(--text); border-radius: 20px; padding: 4px 12px; font-size: 12px; cursor: pointer; }
.cat:hover { border-color: var(--accent); color: var(--accent); }
.cat.missing { border-style: dashed; color: var(--muted); background: transparent; }
.cat .plus { margin-left: 3px; opacity: 0.7; }
.cat .cnt { color: var(--muted); }

.body { flex: 1; min-height: 0; overflow-y: auto; }
.text { width: 100%; height: 100%; resize: none; border: none; background: var(--bg); color: var(--text); padding: 20px 24px; font-size: 13.5px; line-height: 1.65; font-family: ui-monospace, "SF Mono", Menlo, monospace; outline: none; }
.imgwrap { padding: 24px; display: flex; justify-content: center; }
.img { max-width: 100%; height: auto; border-radius: 10px; border: 1px solid var(--border); }
.pdf { width: 100%; height: 100%; border: none; }
.htmlview { width: 100%; height: 100%; border: none; background: #fff; }

.landing { padding: 40px 40px 60px; animation: fade .32s ease; }
.lcard { animation: fadeUp .34s ease backwards; }
.lgrid .lcard:nth-child(1){animation-delay:.02s}.lgrid .lcard:nth-child(2){animation-delay:.05s}.lgrid .lcard:nth-child(3){animation-delay:.08s}.lgrid .lcard:nth-child(4){animation-delay:.11s}.lgrid .lcard:nth-child(5){animation-delay:.14s}.lgrid .lcard:nth-child(6){animation-delay:.17s}.lgrid .lcard:nth-child(7){animation-delay:.2s}.lgrid .lcard:nth-child(8){animation-delay:.23s}
.md-body, .imgwrap, .text, .pdf { animation: fade .3s ease; }
.cfgpanel { animation: fadeUp .26s ease; }
.lhead { display: flex; align-items: flex-start; justify-content: space-between; gap: 12px; margin-bottom: 26px; }
.lhead h1 { font-size: 28px; font-weight: 600; margin: 0; letter-spacing: -0.02em; display: flex; align-items: center; gap: 10px; }
.lhead p { font-size: 15px; color: var(--muted); margin: 6px 0 0; }
.backbtn { border: 1px solid var(--border); background: var(--panel); color: var(--muted); border-radius: 7px; padding: 5px 12px; font-size: 12.5px; cursor: pointer; font-family: inherit; margin-bottom: 16px; }
.backbtn:hover { border-color: var(--accent); color: var(--accent); }
.cnt { color: var(--muted); font-weight: 400; }
.drillfilter { display: flex; flex-direction: column; gap: 10px; margin-bottom: 20px; max-width: 720px; }
.dsearch { width: 100%; box-sizing: border-box; border: 1px solid var(--border); background: var(--panel); color: var(--text); border-radius: 9px; padding: 9px 13px; font-size: 13px; font-family: inherit; outline: none; transition: 0.14s; }
.dsearch:focus { border-color: var(--accent); }
.fchips { display: flex; flex-wrap: wrap; gap: 6px; }
.fchip { border: 1px solid var(--border); background: var(--panel); color: var(--muted); border-radius: 999px; padding: 4px 12px; font-size: 12px; cursor: pointer; font-family: inherit; transition: 0.14s; }
.fchip:hover { border-color: var(--accent); color: var(--text); }
.fchip.on { background: var(--accent); border-color: var(--accent); color: #fff; }
.dempty { margin-top: 18px; }
.lgrid { display: grid; grid-template-columns: repeat(auto-fill, minmax(148px, 1fr)); gap: 12px; max-width: 720px; }
.lcard { display: flex; flex-direction: column; align-items: flex-start; gap: 7px; padding: 18px; border: 1px solid var(--border); background: var(--panel); border-radius: 14px; cursor: pointer; text-align: left; transition: 0.14s; }
.lcard:hover { border-color: var(--accent); transform: translateY(-2px); }
.lcard.missing { border-style: dashed; background: transparent; }
.licon { color: var(--accent); }
.llabel { font-size: 15px; font-weight: 500; color: var(--text); }
.lloc { font-size: 11.5px; color: var(--muted); max-width: 100%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; display: inline-flex; align-items: center; gap: 4px; }
.lstate { font-size: 12px; color: var(--muted); }
.lcard.missing .lstate { color: var(--accent); }

.cfgoverlay { position: fixed; inset: 0; background: rgba(0,0,0,0.35); display: flex; align-items: center; justify-content: center; z-index: 100; }
.cfgpanel { background: var(--panel); border: 1px solid var(--border); border-radius: 14px; padding: 20px 22px; width: 500px; max-width: 90vw; max-height: 82vh; overflow-y: auto; }
.cfghead { display: flex; align-items: center; justify-content: space-between; margin-bottom: 6px; }
.cfghead h3 { margin: 0; font-size: 16px; font-weight: 600; }
.cfghint { font-size: 12.5px; color: var(--muted); margin: 0 0 14px; line-height: 1.5; }
.cfghint code { background: var(--code); padding: 1px 5px; border-radius: 4px; font-size: 0.9em; }
.cfgrow { padding: 12px 0; border-top: 1px solid var(--border); }
.cfgtop { display: flex; align-items: center; gap: 8px; margin-bottom: 9px; }
.cfgcur { color: var(--accent); display: flex; }
.cfgname { flex: 1; font-size: 13px; font-weight: 500; }
.cfgdel { border: none; background: transparent; color: var(--danger); font-size: 11.5px; cursor: pointer; }
.cfgicons { display: flex; flex-wrap: wrap; gap: 4px; align-items: center; margin-bottom: 9px; }
.cfgicon { display: flex; align-items: center; justify-content: center; width: 28px; height: 28px; border: 1px solid var(--border); background: var(--panel); color: var(--muted); border-radius: 7px; cursor: pointer; }
.cfgicon:hover { color: var(--accent); border-color: var(--accent); }
.cfgicon.on { background: var(--accent-soft); color: var(--accent); border-color: transparent; }
.cfgemoji { width: 60px; font-size: 12px; padding: 4px 8px; }
.cfgchips { display: flex; flex-wrap: wrap; gap: 5px; align-items: center; }
.cfgchip { display: inline-flex; align-items: center; gap: 4px; background: var(--accent-soft); color: var(--accent); border-radius: 6px; padding: 3px 4px 3px 8px; font-size: 12px; }
.cfgchip .cx { cursor: pointer; opacity: 0.6; font-size: 14px; padding: 0 2px; }
.cfgchip .cx:hover { opacity: 1; }
.cfginput { flex: 1; min-width: 130px; font-size: 12px; padding: 4px 8px; }
.cfgadd { display: flex; gap: 6px; margin-top: 16px; padding-top: 14px; border-top: 1px solid var(--border); }
.cfgadd input { flex: 1; }
.cfgcolors { padding: 4px 0 14px; }
.swatches { display: flex; align-items: center; gap: 7px; margin-top: 8px; flex-wrap: wrap; }
.swatch { width: 24px; height: 24px; border-radius: 50%; border: none; cursor: pointer; padding: 0; box-shadow: 0 0 0 1px var(--border); }
.swatch.on { box-shadow: 0 0 0 2px var(--panel), 0 0 0 4px var(--accent); }
.colorpick { width: 26px; height: 26px; padding: 0; border: 1px solid var(--border); border-radius: 6px; cursor: pointer; background: none; }
.cfgreset { border: none; background: transparent; color: var(--muted); font-size: 11.5px; cursor: pointer; margin-left: 4px; }
.cfgreset:hover { color: var(--accent); }
.lheadbtns { display: flex; gap: 8px; flex: none; }
.genai { border-color: var(--accent); color: var(--accent); }
.genai:hover { background: var(--accent-soft); }
.aipanel { width: 640px; }
.aistep { padding: 12px 0; border-top: 1px solid var(--border); }
.aisteplabel { font-size: 12px; color: var(--muted); font-weight: 500; margin-bottom: 8px; }
.aitext { width: 100%; height: 150px; resize: vertical; border: 1px solid var(--border); border-radius: 10px; background: var(--code); color: var(--text); padding: 12px 14px; font-size: 12px; line-height: 1.55; font-family: ui-monospace, "SF Mono", Menlo, monospace; outline: none; margin-bottom: 8px; }
.aitext:focus { border-color: var(--accent); }
.airow { display: flex; align-items: center; gap: 10px; }
.ctxback { position: fixed; inset: 0; z-index: 200; }
.ctxmenu { position: fixed; z-index: 201; background: var(--panel); border: 1px solid var(--border); border-radius: 9px; padding: 4px; min-width: 140px; box-shadow: 0 8px 24px rgba(0,0,0,0.15); }
.ctxmenu button { display: block; width: 100%; text-align: left; border: none; background: transparent; color: var(--text); padding: 7px 12px; border-radius: 6px; font-size: 13px; cursor: pointer; font-family: inherit; }
.ctxmenu button:hover { background: var(--sidebar); }
.ctxmenu button.del:hover { color: var(--danger); }
.toast { position: fixed; bottom: 20px; right: 20px; z-index: 300; background: var(--text); color: var(--bg); padding: 10px 16px; border-radius: 10px; font-size: 13px; font-weight: 500; box-shadow: 0 8px 24px rgba(0,0,0,0.2); animation: fadeUp .2s ease; }

.md-body { padding: 8px 32px 40px; max-width: 820px; font-size: 14.5px; line-height: 1.7; }
.md-body h1 { font-size: 24px; margin: 24px 0 12px; }
.md-body h2 { font-size: 19px; margin: 22px 0 10px; padding-bottom: 5px; border-bottom: 1px solid var(--border); }
.md-body h3 { font-size: 16px; margin: 18px 0 8px; }
.md-body a { color: var(--accent); }
.md-body code { background: var(--code); padding: 1px 5px; border-radius: 4px; font-family: ui-monospace, Menlo, monospace; font-size: 0.88em; }
.md-body pre { background: var(--code); padding: 14px 16px; border-radius: 10px; overflow-x: auto; }
.md-body pre code { background: none; padding: 0; }
.md-body img { max-width: 100%; border-radius: 8px; border: 1px solid var(--border); margin: 8px 0; }
.imgmissing { display: inline-block; margin: 8px 0; padding: 14px 18px; border: 1px dashed var(--border); border-radius: 10px; color: var(--muted); font-size: 12.5px; background: var(--sidebar); }
.mermaid-diagram { margin: 16px 0; padding: 16px; border: 1px solid var(--border); border-radius: 12px; background: var(--panel); overflow-x: auto; text-align: center; }
.mermaid-diagram svg { max-width: 100%; height: auto; }
.md-body blockquote { border-left: 3px solid var(--accent); margin: 12px 0; padding: 2px 14px; color: var(--muted); }
.md-body table { border-collapse: collapse; margin: 12px 0; }
.md-body th, .md-body td { border: 1px solid var(--border); padding: 6px 12px; font-size: 13.5px; }
.md-body ul, .md-body ol { padding-left: 22px; }
.md-body hr { border: none; border-top: 1px solid var(--border); margin: 20px 0; }
.md-body input[type="checkbox"] { cursor: pointer; accent-color: var(--accent); width: 15px; height: 15px; vertical-align: -2px; margin-right: 7px; }
.md-body li.task-list-item { list-style: none; margin-left: -20px; }
.md-body pre { position: relative; }
.codecopy { position: absolute; top: 8px; right: 8px; border: 1px solid var(--border); background: var(--panel); color: var(--muted); border-radius: 6px; padding: 2px 8px; font-size: 11px; cursor: pointer; opacity: 0; font-family: inherit; transition: 0.15s; }
.md-body pre:hover .codecopy { opacity: 1; }
.codecopy:hover { color: var(--accent); border-color: var(--accent); }
.editwrap { height: 100%; display: flex; flex-direction: column; }
.edtoolbar { display: flex; gap: 2px; padding: 6px 18px; border-bottom: 1px solid var(--border); flex-wrap: wrap; }
.edtoolbar button { border: none; background: transparent; color: var(--muted); cursor: pointer; min-width: 28px; height: 26px; border-radius: 6px; font-size: 12.5px; font-family: inherit; display: flex; align-items: center; justify-content: center; padding: 0 7px; }
.edtoolbar button:hover { background: var(--sidebar); color: var(--accent); }
.edtoolbar .tb { font-weight: 700; }
.edtoolbar .ti { font-style: italic; }
.editwrap .text { flex: 1; height: auto; min-height: 0; }

.welcome { height: 100%; display: flex; flex-direction: column; align-items: center; justify-content: center; text-align: center; gap: 4px; color: var(--muted); }
.welcome h2 { color: var(--text); font-size: 18px; font-weight: 600; margin: 4px 0 0; }
.welcome p { font-size: 13.5px; max-width: 360px; margin: 0; }
</style>
