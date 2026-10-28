// Explicaciones de cada opción. Tono: alguien con nivel medio de Windows.
// `risk`: safe (no puede dañar nada), reversible (se puede deshacer), care (revisa antes), final (no se deshace).
export const H = {
  cleanButton: {
    risk: 'reversible',
    text: 'Mueve a la cuarentena los archivos de las categorías marcadas. No se borran: puedes devolverlos a su sitio desde la pestaña Cuarentena hasta que la vacíes.',
  },
  cleanRescan: {
    risk: 'safe',
    text: 'Vuelve a medir cuánto ocupa cada categoría. Solo lee; no cambia nada.',
  },
  cleanSafe: {
    risk: 'safe',
    text: '«Seguro» significa que viene marcado por defecto. Las categorías sin marcar son más específicas o pueden ralentizar un poco la primera vez que abras algo.',
  },

  quarantineRestore: {
    risk: 'safe',
    text: 'Devuelve cada archivo de esa limpieza a su carpeta original. Si ya existe un archivo con el mismo nombre allí, no lo sobrescribe.',
  },
  quarantineDelete: {
    risk: 'final',
    text: 'Borra para siempre esa limpieza. Es lo único de WinCleaner que no se puede deshacer. Hazlo cuando estés seguro de que todo funciona bien.',
  },
  quarantineAll: {
    risk: 'final',
    text: 'Borra para siempre todo lo que hay en la cuarentena y libera ese espacio. No se puede deshacer.',
  },
  quarantineOpen: {
    risk: 'safe',
    text: 'Abre en el Explorador la carpeta donde se guardan los archivos limpiados. Mejor no modificar nada dentro a mano.',
  },

  spaceMap: {
    risk: 'safe',
    text: 'Cada bloque es una carpeta o archivo; cuanto mayor, más espacio ocupa. Haz clic en una carpeta para entrar. Lo que marques con el cuadrado se mueve a la cuarentena (no se borra).',
  },
  spaceLargest: {
    risk: 'safe',
    text: 'Los archivos más grandes de esta carpeta. «Mostrar» los abre en el Explorador para que decidas tú qué hacer con ellos.',
  },
  spaceSkipped: {
    risk: 'safe',
    text: 'Carpetas que Windows no deja leer sin permisos de administrador. No se cuentan en el total.',
  },

  spaceDelete: {
    risk: 'reversible',
    text: 'Mueve lo marcado a la cuarentena en vez de borrarlo: los archivos siguen existiendo y puedes devolverlos a su sitio desde la pestaña Cuarentena. Los elementos del sistema (Windows, Archivos de programa, etc.) están bloqueados y no se pueden marcar.',
  },
  spaceProtected: {
    risk: 'care',
    text: 'Este elemento pertenece a Windows o es una carpeta de la que dependen tus programas. WinCleaner no permite moverlo ni borrarlo para que no puedas dañar el sistema por error.',
  },
  startupToggle: {
    risk: 'reversible',
    text: 'Elige si este programa se inicia solo al encender el equipo. «No» solo lo desactiva: el programa sigue instalado y puedes volver a activarlo. Desactivar apps de arranque acelera el inicio.',
  },
  startupUser: {
    risk: 'reversible',
    text: 'Programa que arranca solo para tu usuario (clave «Run» del registro). Desactivarlo es seguro; lo peor que pasa es que deja de abrirse solo.',
  },
  startupMachine: {
    risk: 'care',
    text: 'Arranca para todos los usuarios del equipo y cambiarlo exige permisos de administrador. Suele ser software de controladores o seguridad: si no sabes qué es, déjalo como está.',
  },
  startupFolder: {
    risk: 'reversible',
    text: 'Acceso directo en tu carpeta de Inicio. Desactivarlo solo evita que se abra al encender; el archivo no se toca.',
  },

  uninstallButton: {
    risk: 'care',
    text: 'Abre el desinstalador del propio programa, como en «Aplicaciones» de Windows. Después WinCleaner busca restos y te los enseña para que elijas. La desinstalación del programa en sí no se puede deshacer (habría que reinstalarlo).',
  },
  uninstallPoint: {
    risk: 'safe',
    text: 'Antes de empezar, Windows guarda una foto de su configuración para poder volver a ella desde «Restaurar sistema». Pide ejecutar como administrador y Windows solo permite uno cada 24 horas. No restaura los archivos personales.',
  },
  leftoverFolder: {
    risk: 'reversible',
    text: 'Carpeta con el nombre del programa que sigue en el disco. Al quitarla va a la cuarentena y puedes devolverla. Marca solo las que reconozcas.',
  },
  leftoverRegistry: {
    risk: 'care',
    text: 'Clave del registro de Windows con el nombre del programa. Antes de borrarla se guarda una copia (.reg) y puedes usar «Restaurar registro». Si no sabes qué es, déjala: casi nunca molesta.',
  },

  debloatRemove: {
    risk: 'reversible',
    text: 'Quita las apps marcadas solo para tu usuario. Se pueden reinstalar desde Microsoft Store. No toca componentes del sistema.',
  },
  debloatSafe: {
    risk: 'safe',
    text: 'Marca las apps «Seguras»: aquellas que casi nadie echa de menos. Las «Con cuidado» las dejas tú a mano tras leer qué pasa.',
  },
  debloatPoint: {
    risk: 'safe',
    text: 'Guarda una foto de la configuración de Windows antes de quitar apps. Ojo: las apps de la Tienda no vuelven con el punto de restauración, se reinstalan desde Microsoft Store.',
  },

  restoreCreate: {
    risk: 'safe',
    text: 'Guarda una foto de la configuración de Windows (registro, controladores, ajustes) para poder volver a este momento. No copia tus documentos ni fotos. Requiere administrador y Windows solo permite uno cada 24 horas.',
  },
  restoreAdmin: {
    risk: 'care',
    text: 'Cierra WinCleaner y lo vuelve a abrir con permisos de administrador (Windows te pedirá confirmación). Con ellos también podrás limpiar carpetas del sistema.',
  },
  restoreOpen: {
    risk: 'care',
    text: 'Abre el asistente de Windows para volver a un punto anterior. Al restaurar, se deshacen los cambios de configuración e instalaciones posteriores a ese punto y el equipo se reinicia.',
  },
};
