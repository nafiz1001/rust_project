fn main() {
    windows::build! {
        Windows::Win32::System::ProcessStatus::K32EnumProcesses,
        Windows::Win32::System::Threading::OpenProcess,
        Windows::Win32::Foundation::{HANDLE, HINSTANCE, MAX_PATH, CloseHandle, INVALID_HANDLE_VALUE},
        Windows::Win32::System::ProcessStatus::{K32EnumProcessModulesEx, K32GetModuleBaseNameW},
        Windows::Win32::System::Diagnostics::Debug::{ReadProcessMemory, WriteProcessMemory},
        Windows::Win32::System::Diagnostics::ToolHelp::{CreateToolhelp32Snapshot, Module32FirstW, Module32NextW, MODULEENTRY32W},
    };
}
