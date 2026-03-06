//! Detecção de processos que travam arquivos via Windows Restart Manager API.
//!
//! Usa `RmStartSession` / `RmRegisterResources` / `RmGetList` / `RmEndSession`
//! para identificar quais processos mantêm handles abertos em arquivos específicos.
//! É a mesma API usada pelo Windows Installer para detectar conflitos de arquivo.

use std::mem;
use std::ptr;

// ─── FFI: Restart Manager (rstrtmgr.dll) ───────────────────────────────────

#[allow(non_snake_case, non_camel_case_types, dead_code)]
mod ffi {
    pub const CCH_RM_MAX_APP_NAME: usize = 255;
    pub const CCH_RM_MAX_SVC_NAME: usize = 63;

    /// ERROR_MORE_DATA — buffer muito pequeno, `pnProcInfoNeeded` contém o tamanho necessário.
    pub const ERROR_MORE_DATA: u32 = 234;

    #[repr(C)]
    #[derive(Clone, Copy)]
    #[allow(clippy::upper_case_acronyms)]
    pub struct FILETIME {
        pub dwLowDateTime: u32,
        pub dwHighDateTime: u32,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct RM_UNIQUE_PROCESS {
        pub dwProcessId: u32,
        pub ProcessStartTime: FILETIME,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct RM_PROCESS_INFO {
        pub Process: RM_UNIQUE_PROCESS,
        pub strAppName: [u16; CCH_RM_MAX_APP_NAME + 1],
        pub strServiceShortName: [u16; CCH_RM_MAX_SVC_NAME + 1],
        pub ApplicationType: u32,
        pub AppStatus: u32,
        pub TSSessionId: u32,
        pub bRestartable: i32,
    }

    #[link(name = "rstrtmgr")]
    extern "system" {
        pub fn RmStartSession(
            pSessionHandle: *mut u32,
            dwSessionFlags: u32,
            strSessionKey: *mut u16,
        ) -> u32;

        pub fn RmRegisterResources(
            dwSessionHandle: u32,
            nFiles: u32,
            rgsFileNames: *const *const u16,
            nApplications: u32,
            rgApplications: *const RM_UNIQUE_PROCESS,
            nServices: u32,
            rgsServiceNames: *const *const u16,
        ) -> u32;

        pub fn RmGetList(
            dwSessionHandle: u32,
            pnProcInfoNeeded: *mut u32,
            pnProcInfo: *mut u32,
            rgAffectedApps: *mut RM_PROCESS_INFO,
            lpdwRebootReasons: *mut u32,
        ) -> u32;

        pub fn RmEndSession(dwSessionHandle: u32) -> u32;
    }
}

// ─── Tipos públicos ─────────────────────────────────────────────────────────

/// Processo que está travando um arquivo.
#[derive(Debug, Clone, serde::Serialize)]
pub struct LockingProcess {
    /// PID do processo
    pub pid: u32,
    /// Nome do processo (ex: "chrome", "explorer")
    pub name: String,
}

// ─── API pública ────────────────────────────────────────────────────────────

/// Identifica quais processos estão usando o arquivo indicado.
///
/// Retorna `Vec` vazio se nenhum processo estiver travando o arquivo
/// ou se a API falhar (falha silenciosa — não é crítico).
pub fn get_locking_processes(file_path: &str) -> Vec<LockingProcess> {
    use std::os::windows::ffi::OsStrExt;

    unsafe {
        let mut session: u32 = 0;
        // CCH_RM_SESSION_KEY = 32 chars + null
        let mut session_key = vec![0u16; 64];

        if ffi::RmStartSession(&mut session, 0, session_key.as_mut_ptr()) != 0 {
            return Vec::new();
        }

        // Converte o caminho para wide string (UTF-16 + null terminator)
        let wide_path: Vec<u16> = std::ffi::OsStr::new(file_path)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let path_ptr: *const u16 = wide_path.as_ptr();

        if ffi::RmRegisterResources(session, 1, &path_ptr, 0, ptr::null(), 0, ptr::null()) != 0 {
            ffi::RmEndSession(session);
            return Vec::new();
        }

        // Primeira chamada: descobre quantos processos usam o arquivo
        let mut needed: u32 = 0;
        let mut count: u32 = 0;
        let mut reboot_reasons: u32 = 0;

        let result = ffi::RmGetList(
            session,
            &mut needed,
            &mut count,
            ptr::null_mut(),
            &mut reboot_reasons,
        );

        if result != ffi::ERROR_MORE_DATA && result != 0 {
            ffi::RmEndSession(session);
            return Vec::new();
        }

        if needed == 0 {
            ffi::RmEndSession(session);
            return Vec::new();
        }

        // Segunda chamada: obtém os dados dos processos
        count = needed;
        let mut processes = vec![mem::zeroed::<ffi::RM_PROCESS_INFO>(); count as usize];

        if ffi::RmGetList(
            session,
            &mut needed,
            &mut count,
            processes.as_mut_ptr(),
            &mut reboot_reasons,
        ) != 0
        {
            ffi::RmEndSession(session);
            return Vec::new();
        }

        ffi::RmEndSession(session);

        processes.truncate(count as usize);
        processes
            .iter()
            .map(|p| {
                let name_len = p
                    .strAppName
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(p.strAppName.len());
                LockingProcess {
                    pid: p.Process.dwProcessId,
                    name: String::from_utf16_lossy(&p.strAppName[..name_len]),
                }
            })
            .collect()
    }
}
