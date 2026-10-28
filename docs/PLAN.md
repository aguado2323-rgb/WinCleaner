# WinCleaner: plan de proyecto

Escala de esfuerzo: **S** = 1-2 días, **M** = 3-7 días, **L** = 2-3 semanas, **XL** = más de 3 semanas (ritmo de una persona con Claude).
Conveniencia: ★★★ imprescindible, ★★ aporta mucho, ★ opcional. Riesgo = daño posible al usuario si falla.

## 1. Dónde estamos (v0.1, ya en `main`)

| Pieza | Estado |
|---|---|
| Núcleo Rust: reglas TOML, escaneo, limpieza con dry-run, rechazo de raíces, sin seguir enlaces simbólicos | Hecho, 4 tests |
| 7 reglas: temp usuario, temp Windows, miniaturas, volcados de error, caché Chrome/Edge/Firefox | Hecho |
| Shell Tauri 2 con comandos `list_rules`, `scan`, `clean` | Escrito, **sin compilar** (lo verificará el CI) |
| UI Svelte: lista de categorías, analizar, limpiar, tema claro/oscuro | Compila; sin probar en la ventana real |
| CI en Windows, README, roadmap, skills activadas | Hecho |

**Huecos conocidos del v0.1:** falta icono; "Limpiar" no pide confirmación ni muestra qué borra; las reglas de Firefox y Windows Temp tienen casos límite (perfiles, permisos de administrador); no se detectan archivos en uso (se informan como errores); no hay manejo de errores en la UI.

## 2. Catálogo de features

| # | Feature | Esfuerzo | Conveniencia | Riesgo | Notas |
|---|---|---|---|---|---|
| F1 | Endurecer limpiador: confirmación, lista detallada, errores en UI, archivos en uso, elevación a admin | M | ★★★ | Medio | Base de confianza de todo lo demás |
| F2 | Más reglas: Prefetch, Windows Update cache, logs, Papelera, DNS, recientes, cachés de apps (Discord, Spotify, Teams, VS Code), dev (npm, pip, NuGet, cargo) | M | ★★★ | Bajo | Solo datos TOML + tests; escala con la comunidad |
| F3 | Navegadores completos: Brave, Opera, Vivaldi, perfiles múltiples, historial/cookies opcionales (desmarcados) | M | ★★ | Bajo | Detectar perfiles reales |
| F4 | Analizador de espacio (treemap), archivos grandes | M | ★★★ | Nulo | Solo lectura; gran valor percibido, diferencia frente a CCleaner |
| F5 | Gestor de arranque (Run, Startup, tareas programadas, servicios) con impacto | M | ★★★ | Bajo | Desactivar, no borrar |
| F6 | Desinstalador: lista de apps (registro + Tienda), desinstalar con el instalador oficial | M | ★★★ | Medio | |
| F7 | Desinstalador: restos (carpetas, claves de registro, tareas) con copia `.reg` previa y revisión manual | L | ★★★ | Alto | Es el corazón del tipo "Revo"; hacerlo conservador |
| F8 | Desinstalación por lotes y bloatware (perfiles seguros) | M | ★★ | Medio | Depende de F6 |
| F9 | Cuarentena/deshacer: mover a carpeta propia N días en vez de borrar | M | ★★★ | Bajo | Reduce el riesgo de F1, F7, F10; conviene hacerlo pronto |
| F10 | Buscador de duplicados (tamaño, hash parcial, hash completo) | L | ★★ | Medio | Nunca autoseleccionar; usar cuarentena |
| F11 | Triturador de archivos y espacio libre | M | ★ | Medio | Poco útil en SSD; baja prioridad |
| F12 | Historial de limpiezas y estadísticas | S | ★★ | Nulo | Local, sin telemetría |
| F13 | Programación (tarea programada de Windows) y limpieza silenciosa | M | ★★ | Bajo | Mejor usar el Programador de tareas que un servicio residente (RAM) |
| F14 | Actualizador de software vía winget | M | ★★ | Bajo | Envolver `winget upgrade` |
| F15 | Monitor de RAM/CPU/disco y bandeja | M | ★ | Bajo | Choca con el objetivo de poca RAM; opcional y desactivado |
| F16 | Privacidad: extensiones sospechosas, rastros de Windows | M | ★ | Bajo | |
| F17 | Limpiador de registro | L | ★ (no hacer) | Alto | Beneficio casi nulo; descartado salvo restos del desinstalador |
| F18 | Instalador NSIS + versión portable, firma de código, releases automáticos, autoactualización | L | ★★★ | Bajo | Firma cuesta dinero (SmartScreen); ver riesgos |
| F19 | i18n (ES/EN), accesibilidad, pulido visual, onboarding | M | ★★ | Nulo | |
| F20 | CLI (`wincleaner clean --dry-run`) | S | ★★ | Bajo | Reutiliza el núcleo casi gratis |
| F21 | Medición y presupuesto de RAM/peso en CI | S | ★★★ | Nulo | Garantiza la promesa; falla el CI si se supera |

## 3. Plan por fases

**Fase 0: Cimientos (S, esta semana).** Compilar y probar Tauri en Windows, icono, arreglar lo que salga del CI, F21 (presupuesto de peso/RAM: objetivo instalador menos de 10 MB, RAM en reposo menos de 60 MB con ventana abierta, menos de 10 MB en segundo plano).
*Salida:* v0.1.1 que arranca en Windows.

**Fase 1: Limpiador fiable (v0.2, ~2 semanas).** F1, F9, F2, F12.
*Salida:* limpiar con vista previa, confirmación, cuarentena y reglas amplias. Ya es un producto útil.

**Fase 2: Ver y controlar (v0.3, ~2 semanas).** F4, F5, F3.
*Salida:* espacio en disco y arranque, las dos funciones más pedidas junto con la limpieza.

**Fase 3: Desinstalador (v0.4, ~3 semanas).** F6, F7, F8.
*Salida:* equivalente a Revo en lo esencial. Salir primero con "desinstalar + restos revisables" y sin autoborrado.

**Fase 4: Distribución (v0.5, ~2 semanas).** F18, F19, F20.
*Salida:* instalador, portable, releases en GitHub, ES/EN. Aquí conviene abrir beta pública.

**Fase 5: v1.0 (~3 semanas).** F10, F13, F14, pulido, documentación y auditoría de seguridad de todo lo que borra.

**Después (según demanda):** F16, F15 opcional, F11. No se hará F17.

Total estimado hasta v1.0: unas 12-14 semanas en solitario con Claude, menos si se recorta F10/F14.

## 4. Calidad y skills por fase

- **Cada fase:** `quality-assurance-suite` (revisión de entrega y auditoría de tests antes de fusionar).
- **Fase 0-1:** `architecture-suite` para fijar ADRs (por qué Tauri, formato de reglas, política de cuarentena).
- **Fase 2-3:** `product-discovery-suite` para requisitos y flujos del desinstalador.
- **Rendimiento:** `implementation-suite` (optimizador y benchmark) para defender el presupuesto de RAM/peso.
- Pruebas de lo destructivo siempre con directorios temporales; ninguna regla se acepta sin test.

## 5. Riesgos

| Riesgo | Mitigación |
|---|---|
| Borrar algo que el usuario necesita | Dry-run, cuarentena (F9), reglas conservadoras, `safe=false` desmarcado |
| Aviso de SmartScreen sin firma | Empezar con binarios sin firmar y checksums; valorar certificado o Azure Trusted Signing al llegar a la beta |
| No poder probar en Windows desde la nube | CI en `windows-latest` como validación; pruebas manuales tuyas en cada hito |
| WebView2 sube la RAM | Cerrar la ventana al ir a bandeja o no usar residente (F13 con tarea programada); medir en F21 |
| Reglas obsoletas al cambiar las apps | Reglas en TOML, fácil de aportar y actualizar |
| Falsos positivos de antivirus por borrar/tocar registro | Código abierto, builds reproducibles, evitar comportamientos de malware (sin persistencia agresiva) |

## 6. Decisiones que necesito de ti

1. ¿Distribución gratuita y de código abierto (recomendado: licencia MIT, es la mejor baza frente a CCleaner) o cerrada?
2. ¿Empezamos por Fase 0 y 1 como está, o prefieres adelantar el analizador de espacio (F4) por ser lo más vistoso?
3. ¿Tienes un Windows para probar cada hito?
