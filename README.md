# WinCleaner

Limpiador y optimizador para Windows: sin anuncios, sin telemetría, ligero y con interfaz cuidada.

## Estado
v0.1 (esqueleto): limpieza de temporales y cachés de navegadores con análisis previo (dry-run).
Plan completo de alcance en `docs/ROADMAP.md`.

## Stack
Tauri 2 + Rust (núcleo en `crates/core`) + Svelte 5 (`ui`).
Las reglas de limpieza son declarativas: `crates/core/rules/windows.toml`.

## Desarrollo
```
npm install --prefix ui
cargo test -p wincleaner-core     # funciona en cualquier SO
cargo tauri dev                   # requiere Windows + WebView2 (cargo install tauri-cli)
```
Falta añadir `src-tauri/icons/icon.ico` antes de empaquetar (`cargo tauri icon`).

## Skills de Claude Code
`.claude/settings.json` activa architecture, product-discovery, quality-assurance e implementation suites de
[levnikolaevich/claude-code-skills](https://github.com/levnikolaevich/claude-code-skills).
