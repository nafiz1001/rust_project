fn main() {
    windows::build! {
        Windows::Win32::System::ProcessStatus::K32EnumProcesses,
        Windows::Win32::System::Threading::OpenProcess,
        Windows::Win32::Foundation::{MAX_PATH, CloseHandle, INVALID_HANDLE_VALUE},
        Windows::Win32::System::ProcessStatus::{K32EnumProcessModulesEx, K32GetModuleBaseNameW},
        Windows::Win32::System::Diagnostics::Debug::{ReadProcessMemory, WriteProcessMemory},
        Windows::Win32::System::Diagnostics::ToolHelp::{CreateToolhelp32Snapshot, Module32FirstW, Module32NextW, Process32FirstW, Process32NextW},
        Windows::Win32::System::Memory::{VirtualQueryEx, MEMORY_BASIC_INFORMATION32, MEMORY_BASIC_INFORMATION64},
        Windows::Win32::System::SystemInformation::{GetSystemInfo, SYSTEM_INFO},
    };
}
