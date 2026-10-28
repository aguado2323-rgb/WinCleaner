# Publicar una versión

1. Sube la versión en `src-tauri/Cargo.toml` y `src-tauri/tauri.conf.json` (por ejemplo `0.2.0`) y fusiona el cambio en `main`.
2. En GitHub: **Releases → Draft a new release**, crea la etiqueta `v0.2.0` sobre `main`, escribe las notas y pulsa **Publish release**.
3. La acción **Release** compila el exe, lo adjunta como `WinCleaner-v0.2.0-portable.exe` junto a `SHA256SUMS.txt` y añade el SHA256 a las notas. Tarda unos 5 minutos.

Comprobar una descarga en PowerShell:

```powershell
(Get-FileHash .\WinCleaner-v0.2.0-portable.exe).Hash
```

Debe coincidir con el SHA256 de las notas.

## Firma de código (opcional, gratis para código abierto)

Sin firma, Chrome marca la descarga como poco común y Windows SmartScreen muestra «Windows protegió su PC». Con la firma desaparece el aviso del editor desconocido (SmartScreen sigue pidiendo algo de reputación al principio).

1. Solicita el programa gratuito en <https://signpath.org/foundation> (SignPath Foundation, para proyectos de código abierto con licencia aprobada; WinCleaner es MIT). Indica el repositorio, que compilas con GitHub Actions y que no incluyes adware. La respuesta puede tardar días; las condiciones las fija SignPath.
2. Cuando te den acceso, en SignPath crea un proyecto y una política de firma, y copia su token de API.
3. En GitHub: **Settings → Secrets and variables → Actions**:
   - Secreto `SIGNPATH_API_TOKEN`: el token.
   - Variables `SIGNPATH_ORGANIZATION_ID`, `SIGNPATH_PROJECT_SLUG`, `SIGNPATH_POLICY_SLUG`.
4. La próxima vez que publiques una versión, el paso «Firmar con SignPath» se activa solo y el exe sale firmado.

Mientras no existan esas variables el paso se omite y el exe se publica sin firmar, con su SHA256.
