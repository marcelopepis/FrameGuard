// Verificação de privilégios de administrador via Windows API

use std::ffi::c_void;
use std::mem;
use std::ptr;

// Bindings mínimos para OpenProcessToken + GetTokenInformation
mod ffi {
    use std::ffi::c_void;

    #[link(name = "advapi32")]
    extern "system" {
        pub fn OpenProcessToken(
            ProcessHandle: *mut c_void,
            DesiredAccess: u32,
            TokenHandle: *mut *mut c_void,
        ) -> i32;

        pub fn GetTokenInformation(
            TokenHandle: *mut c_void,
            TokenInformationClass: u32,
            TokenInformation: *mut c_void,
            TokenInformationLength: u32,
            ReturnLength: *mut u32,
        ) -> i32;
    }

    #[link(name = "kernel32")]
    extern "system" {
        pub fn GetCurrentProcess() -> *mut c_void;
        pub fn CloseHandle(hObject: *mut c_void) -> i32;
    }
}

/// Verifica se o processo atual está rodando com privilégios de administrador.
/// Usa OpenProcessToken + TokenElevation via Windows API.
pub fn is_elevated() -> bool {
    const TOKEN_QUERY: u32 = 0x0008;
    const TOKEN_ELEVATION: u32 = 20;

    #[repr(C)]
    struct TokenElevation {
        token_is_elevated: u32,
    }

    unsafe {
        let process = ffi::GetCurrentProcess();
        let mut token: *mut c_void = ptr::null_mut();

        if ffi::OpenProcessToken(process, TOKEN_QUERY, &mut token) == 0 {
            return false;
        }

        let mut elevation = TokenElevation { token_is_elevated: 0 };
        let mut return_len: u32 = 0;

        let ok = ffi::GetTokenInformation(
            token,
            TOKEN_ELEVATION,
            &mut elevation as *mut _ as *mut c_void,
            mem::size_of::<TokenElevation>() as u32,
            &mut return_len,
        );

        ffi::CloseHandle(token);

        ok != 0 && elevation.token_is_elevated != 0
    }
}
