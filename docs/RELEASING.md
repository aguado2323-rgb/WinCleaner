# Publicar una versión

1. En GitHub: **Releases → Draft a new release**, crea la etiqueta `v0.2.0` sobre `main`, escribe las notas y pulsa **Publish release**.
2. La acción **Release** pone al programa la versión de la etiqueta (la etiqueta tiene que ser `vX.Y.Z`), compila el exe, lo adjunta como `WinCleaner-v0.2.0-portable.exe` junto a `SHA256SUMS.txt` y su firma `.sig`, y añade el SHA256 a las notas. Tarda unos 5 minutos.
3. Las copias de WinCleaner que ya tenga la gente verán la versión nueva al abrirse (sección **Actualizaciones**) y podrán instalarla con un clic.

Comprobar una descarga en PowerShell:

```powershell
(Get-FileHash .\WinCleaner-v0.2.0-portable.exe).Hash
```

Debe coincidir con el SHA256 de las notas.

## Firma de las actualizaciones automáticas

WinCleaner se actualiza solo, sin instalador: descarga el exe nuevo de la última versión de GitHub, comprueba su firma y lo pone en lugar del actual. La firma es Ed25519 (formato minisign, el mismo de Tauri) y no tiene nada que ver con la de SignPath:

- La **clave pública** está en `crates/core/updater.pub` y va dentro del exe.
- La **clave privada** está solo en el secreto de GitHub `TAURI_SIGNING_PRIVATE_KEY` (y su contraseña, si tiene, en `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`).
- El paso «Firmar para las actualizaciones automáticas» firma el exe final y sube `WinCleaner-vX.Y.Z-portable.exe.sig`.
- La app rechaza la descarga si la firma no coincide o si es de otro archivo. Si una versión sale sin `.sig`, la app avisa pero solo ofrece abrir la página de descarga.

Crear las claves (una sola vez, necesita Node.js):

```powershell
npx @tauri-apps/cli@2 signer generate -w wincleaner-update.key
```

Copia el contenido de `wincleaner-update.key` al secreto `TAURI_SIGNING_PRIVATE_KEY` y el de `wincleaner-update.key.pub` a `crates/core/updater.pub`. Guarda la clave privada en un sitio seguro: si se pierde, las copias ya instaladas no podrán actualizarse solas y habrá que descargar la siguiente versión a mano.

## Firma de código (opcional, gratis para código abierto)

Sin firma, Chrome marca la descarga como poco común y Windows SmartScreen muestra «Windows protegió su PC». Con la firma desaparece el aviso del editor desconocido (SmartScreen sigue pidiendo algo de reputación al principio).

1. Solicita el programa gratuito en <https://signpath.org/foundation> (SignPath Foundation, para proyectos de código abierto con licencia aprobada; WinCleaner es MIT). Indica el repositorio, que compilas con GitHub Actions y que no incluyes adware. La respuesta puede tardar días; las condiciones las fija SignPath.
2. Cuando te den acceso, en SignPath crea un proyecto y una política de firma, y copia su token de API.
3. En GitHub: **Settings → Secrets and variables → Actions**:
   - Secreto `SIGNPATH_API_TOKEN`: el token.
   - Variables `SIGNPATH_ORGANIZATION_ID`, `SIGNPATH_PROJECT_SLUG`, `SIGNPATH_POLICY_SLUG`.
4. La próxima vez que publiques una versión, el paso «Firmar con SignPath» se activa solo y el exe sale firmado.

Mientras no existan esas variables el paso se omite y el exe se publica sin firmar, con su SHA256.
