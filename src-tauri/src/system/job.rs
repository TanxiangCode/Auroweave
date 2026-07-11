/// Windows 作业对象 (Job Object) 封装 — 确保父子进程生命周期强绑定
/// 作者: TanXiang

#[cfg(target_os = "windows")]
mod win {
    use std::os::windows::io::RawHandle;
    use std::ptr;

    #[link(name = "kernel32")]
    extern "system" {
        fn CreateJobObjectW(
            lpJobAttributes: *mut std::ffi::c_void,
            lpName: *const u16,
        ) -> RawHandle;

        fn SetInformationJobObject(
            hJob: RawHandle,
            JobObjectInformationClass: u32,
            lpJobObjectInformation: *const std::ffi::c_void,
            cbJobObjectInformationLength: u32,
        ) -> i32;

        fn AssignProcessToJobObject(
            hJob: RawHandle,
            hProcess: RawHandle,
        ) -> i32;

        fn CloseHandle(hObject: RawHandle) -> i32;
    }

    #[repr(C)]
    #[allow(non_camel_case_types)]
    struct JOBOBJECT_BASIC_LIMIT_INFORMATION {
        per_process_user_time_limit: i64,
        per_job_user_time_limit: i64,
        limit_flags: u32,
        minimum_working_set_size: usize,
        maximum_working_set_size: usize,
        active_process_limit: u32,
        affinity: usize,
        priority_class: u32,
        scheduling_class: u32,
    }

    #[repr(C)]
    #[allow(non_camel_case_types)]
    struct IO_COUNTERS {
        read_operation_count: u64,
        write_operation_count: u64,
        other_operation_count: u64,
        read_transfer_count: u64,
        write_transfer_count: u64,
        other_transfer_count: u64,
    }

    #[repr(C)]
    #[allow(non_camel_case_types)]
    struct JOBOBJECT_EXTENDED_LIMIT_INFORMATION {
        basic_limit_information: JOBOBJECT_BASIC_LIMIT_INFORMATION,
        io_info: IO_COUNTERS,
        process_memory_limit: usize,
        job_memory_limit: usize,
        peak_process_memory_limit: usize,
        peak_job_memory_limit: usize,
    }

    const JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE: u32 = 0x2000;
    const JOB_OBJECT_INFO_CLASS: u32 = 9; // JobObjectExtendedLimitInformation

    pub struct WinJob {
        handle: RawHandle,
    }

    impl WinJob {
        pub fn create() -> Result<Self, String> {
            unsafe {
                let handle = CreateJobObjectW(ptr::null_mut(), ptr::null());
                if handle.is_null() {
                    return Err("创建 Job Object 失败".to_string());
                }

                let mut info = std::mem::zeroed::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>();
                info.basic_limit_information.limit_flags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;

                let size = std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32;
                let res = SetInformationJobObject(
                    handle,
                    JOB_OBJECT_INFO_CLASS,
                    &info as *const _ as *const std::ffi::c_void,
                    size,
                );

                if res == 0 {
                    CloseHandle(handle);
                    return Err("设置 Job Object 信息失败".to_string());
                }

                Ok(Self { handle })
            }
        }

        pub fn assign_process(&self, process_handle: RawHandle) -> Result<(), String> {
            unsafe {
                let res = AssignProcessToJobObject(self.handle, process_handle);
                if res == 0 {
                    return Err("将进程加入 Job Object 失败".to_string());
                }
                Ok(())
            }
        }
    }

    impl Drop for WinJob {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.handle);
            }
        }
    }
}

// 跨平台外部接口包装
#[cfg(target_os = "windows")]
pub struct JobObject(win::WinJob);

#[cfg(target_os = "windows")]
unsafe impl Send for JobObject {}
#[cfg(target_os = "windows")]
unsafe impl Sync for JobObject {}

#[cfg(target_os = "windows")]
impl JobObject {
    pub fn create() -> Result<Self, String> {
        win::WinJob::create().map(JobObject)
    }

    pub fn assign_process(&self, process_handle: std::os::windows::io::RawHandle) -> Result<(), String> {
        self.0.assign_process(process_handle)
    }
}

#[cfg(not(target_os = "windows"))]
pub struct JobObject;

#[cfg(not(target_os = "windows"))]
impl JobObject {
    pub fn create() -> Result<Self, String> {
        Ok(JobObject)
    }
}
