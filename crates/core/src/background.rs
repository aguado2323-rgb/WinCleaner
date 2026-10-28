//! Prioridad baja de Windows para los análisis largos (recorrer discos, buscar duplicados).
//! El hilo pasa a «modo segundo plano»: menos prioridad de CPU y de disco, así el análisis cede
//! ante lo que la persona esté usando y el equipo no se nota cargado. No cambia lo que se analiza.

/// Ejecuta `f` en el hilo actual con prioridad de segundo plano y la restaura al terminar
/// (los hilos de trabajo se reutilizan, así que nunca se quedan en modo lento).
pub fn low_priority<T>(f: impl FnOnce() -> T) -> T {
    let _guard = imp::Guard::begin();
    f()
}

#[cfg(windows)]
mod imp {
    use std::ffi::c_void;

    const THREAD_MODE_BACKGROUND_BEGIN: i32 = 0x0001_0000;
    const THREAD_MODE_BACKGROUND_END: i32 = 0x0002_0000;

    #[link(name = "kernel32")]
    extern "system" {
        fn GetCurrentThread() -> *mut c_void;
        fn SetThreadPriority(thread: *mut c_void, priority: i32) -> i32;
    }

    /// Si Windows no acepta el cambio (p. ej. el hilo ya estaba en segundo plano), no se toca nada al salir.
    pub struct Guard(bool);

    impl Guard {
        pub fn begin() -> Self {
            // SAFETY: GetCurrentThread devuelve un pseudo-identificador del hilo actual que no hay que cerrar.
            Guard(unsafe { SetThreadPriority(GetCurrentThread(), THREAD_MODE_BACKGROUND_BEGIN) } != 0)
        }
    }

    impl Drop for Guard {
        fn drop(&mut self) {
            if self.0 {
                // SAFETY: mismo hilo que en `begin` (el guard no sale de `low_priority`).
                unsafe {
                    SetThreadPriority(GetCurrentThread(), THREAD_MODE_BACKGROUND_END);
                }
            }
        }
    }
}

#[cfg(not(windows))]
mod imp {
    pub struct Guard;

    impl Guard {
        pub fn begin() -> Self {
            Guard
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn returns_the_result_and_can_nest() {
        assert_eq!(super::low_priority(|| super::low_priority(|| 2 + 2)), 4);
    }
}
