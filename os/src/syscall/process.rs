//! Process management syscalls

use crate::{
    config::MAX_SYSCALL_NUM,
    task::{
        change_program_brk, exit_current_and_run_next, suspend_current_and_run_next, TaskStatus, current_user_token, get_current_time, get_current_sys_count, get_current_status, do_mmap, do_munmap,
    }, timer::{get_time_us, get_time_ms}, mm::{translated_byte_buffer, VirtAddr},
};

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

/// Task information
#[allow(dead_code)]
pub struct TaskInfo {
    /// Task status in it's life cycle
    status: TaskStatus,
    /// The numbers of syscall called by task
    syscall_times: [u32; MAX_SYSCALL_NUM],
    /// Total running time of task
    time: usize,
}

/// task exits and submit an exit code
pub fn sys_exit(_exit_code: i32) -> ! {
    trace!("kernel: sys_exit");
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    trace!("kernel: sys_yield");
    suspend_current_and_run_next();
    0
}

/// YOUR JOB: get time with second and microsecond
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TimeVal`] is splitted by two pages ?
pub fn sys_get_time(ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");
    let usec = get_time_us() % 1_000_000;
    let sec = get_time_ms() / 1_000;
    let tv = TimeVal { sec, usec };
    
    let tv_buffer = unsafe {
        core::slice::from_raw_parts(&tv as *const TimeVal as *const u8, core::mem::size_of::<TimeVal>())
    };
    let ts_buffer = translated_byte_buffer(current_user_token(), ts as *mut u8, core::mem::size_of::<TimeVal>());
    let mut i = 0;
    for bytes in ts_buffer {
        bytes.copy_from_slice(&tv_buffer[i .. i + bytes.len()]);
        i += bytes.len();
    }
    0
}

/// YOUR JOB: Finish sys_task_info to pass testcases
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TaskInfo`] is splitted by two pages ?
pub fn sys_task_info(ti: *mut TaskInfo) -> isize {
    trace!("kernel: sys_task_info");
    let status = get_current_status();
    let syscall_times = get_current_sys_count();
    let time = get_current_time();
    let tis = TaskInfo {
        status,
        syscall_times,
        time,
    };
    let tis_buffer = unsafe {
        core::slice::from_raw_parts(&tis as *const TaskInfo as *const u8, core::mem::size_of::<TaskInfo>())
    };
    let ti_buffer = translated_byte_buffer(current_user_token(), ti as *mut u8, core::mem::size_of::<TaskInfo>());
    let mut i = 0;
    for bytes in ti_buffer {
        bytes.copy_from_slice(&tis_buffer[i .. i + bytes.len()]);
        i += bytes.len();
    }
    0
}

// YOUR JOB: Implement mmap.
pub fn sys_mmap(start: usize, len: usize, port: usize) -> isize {
    trace!("kernel: sys_mmap");
    if !VirtAddr::from(start).aligned() {
        return -1;
    }
    if (port & !0x7usize != 0usize)  || (port & 0x7usize == 0usize) {
        return -1;
    }
    
    do_mmap(start, len, port)
}

// YOUR JOB: Implement munmap.
pub fn sys_munmap(start: usize, len: usize) -> isize {
    trace!("kernel: sys_munmap");
    if !VirtAddr::from(start).aligned() {
        return -1;
    }
    do_munmap(start, len)
}
/// change data segment size
pub fn sys_sbrk(size: i32) -> isize {
    trace!("kernel: sys_sbrk");
    if let Some(old_brk) = change_program_brk(size) {
        old_brk as isize
    } else {
        -1
    }
}
