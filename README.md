# BoredPowerMD 🪐

**Un organizador de documentación local-first para tus proyectos.** BoredPowerMD es una app de escritorio (Tauri + Vue 3 + Rust) que reúne tus README, ideas, arquitectura, políticas de seguridad y notas de cada proyecto en una sola wiki limpia — todo guardado como **archivos reales en tu disco**, listos para git. Sin nube, sin cuentas, sin base de datos: tus documentos siguen siendo tuyos.

> Pensada para desarrolladores que tienen decenas de repos y quieren ver, editar y exportar su documentación sin perderse entre carpetas.

---

## ✨ Características

- **Wiki por proyecto** — apunta a tu carpeta raíz (ej. `~/Documents/GitHub`) y cada subcarpeta se vuelve un proyecto navegable.
- **Categorías inteligentes** — README, Architecture, Security, Product, Contract, Ideas, AI Context y Extras. Se detectan solas por el nombre del archivo y son configurables.
- **Buscador + filtro por formato** dentro de cada categoría, para no saturarte cuando hay muchos archivos.
- **Vista previa rica de Markdown** — con diagramas **Mermaid** (UML, secuencia, flujo), fórmulas **LaTeX/KaTeX**, resaltado de código, casillas interactivas e imágenes locales.
- **Visor de HTML** integrado, con botón *"Abrir en navegador"* para páginas que enlazan entre sí.
- **Visor de PDF e imágenes**.
- **Editor** con barra de herramientas (encabezados, negrita, código, tablas, insertar imágenes desde tu gestor de archivos, LaTeX).
- **Exportar a PDF** (documento imprimible autocontenido, con portada por sección) y **exportar a ZIP** de solo la documentación — tú eliges dónde guardar.
- **Modo claro / oscuro** y color de acento personalizable.
- **Ignorar archivos** con click derecho para dejarlos fuera de las exportaciones.

---

## 📥 Descargar (usuarios)

Ve a la pestaña **[Releases](../../releases)** y descarga el instalador para tu sistema:

| Sistema | Archivo |
|---|---|
| **macOS** (Apple Silicon e Intel) | `.dmg` |
| **Windows** | `.msi` o `-setup.exe` |

> Los builds son sin firma de código, así que la primera vez macOS/Windows pueden pedir confirmación (en Mac: click derecho → *Abrir*).

---

## 🛠️ Desarrollo (correr desde el código)

**Requisitos:** [Node.js](https://nodejs.org) y [Rust](https://www.rust-lang.org/tools/install).

```bash
# 1. Instalar Rust (una sola vez)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 2. Instalar dependencias y correr en modo desarrollo
npm install
npm run tauri dev
```

### Compilar los instaladores localmente

```bash
npm run tauri build
```

Los artefactos quedan en `src-tauri/target/release/bundle/`.

---

## 🧱 Stack y estructura

- **Frontend:** Vue 3 (`<script setup>`) + TypeScript + Vite
- **Backend nativo:** Rust (Tauri 2) — acceso a archivos, generación de ZIP, lectura de imágenes/PDF
- **Docs:** `marked` (Markdown) · `mermaid` (diagramas) · `katex` (LaTeX)

```
src/
  App.vue        ← toda la interfaz
  TreeNode.vue   ← árbol de archivos recursivo
  Icon.vue       ← iconos SVG
src-tauri/
  src/lib.rs     ← comandos nativos (list_dir, read_file, write_file, export_zip, …)
.github/workflows/release.yml  ← compila macOS + Windows automáticamente
```

---

## 📄 Licencia

MIT — ver [LICENSE](LICENSE).
