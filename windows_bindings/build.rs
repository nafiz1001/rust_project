fn main() {
    windows::build! {
        Windows::Win32::System::ProcessStatus::K32EnumProcesses,
        Windows::Win32::System::Threading::OpenProcess,
        Windows::Win32::Foundation::{HANDLE, HINSTANCE, MAX_PATH, CloseHandle},
        Windows::Win32::System::ProcessStatus::{K32EnumProcessModulesEx, K32GetModuleBaseNameW},
    };
}
