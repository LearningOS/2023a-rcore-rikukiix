use crate::sync::{Condvar, Mutex, MutexBlocking, MutexSpin, Semaphore};
use crate::task::{block_current_and_run_next, current_process, current_task};
use crate::timer::{add_timer, get_time_ms};
use alloc::sync::Arc;
use alloc::vec;
/// sleep syscall
pub fn sys_sleep(ms: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_sleep",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let expire_ms = get_time_ms() + ms;
    let task = current_task().unwrap();
    add_timer(expire_ms, task);
    block_current_and_run_next();
    0
}
/// mutex create syscall
pub fn sys_mutex_create(blocking: bool) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_mutex_create",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mutex: Option<Arc<dyn Mutex>> = if !blocking {
        Some(Arc::new(MutexSpin::new()))
    } else {
        Some(Arc::new(MutexBlocking::new()))
    };
    let mut process_inner = process.inner_exclusive_access();
    if let Some(id) = process_inner
        .mutex_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.mutex_list[id] = mutex;
        process_inner.mutex_available[id] = 1;
        if process_inner.mutex_need.is_empty() {
            let mutex_count = process_inner.mutex_list.len();
            process_inner.mutex_allocation.push(vec![0; mutex_count]);
            process_inner.mutex_need.push(vec![0; mutex_count]);
        }
        let _ = process_inner.mutex_allocation.iter_mut().map(|v| v[id] = 0);
        let _ = process_inner.mutex_need.iter_mut().map(|v| v[id] = 0);
        id as isize
    } else {
        if process_inner.mutex_need.is_empty() {
            let mutex_count = process_inner.mutex_list.len();
            process_inner.mutex_allocation.push(vec![0; mutex_count]);
            process_inner.mutex_need.push(vec![0; mutex_count]);
        }
        process_inner.mutex_list.push(mutex);
        process_inner.mutex_available.push(1);
        for v in process_inner.mutex_allocation.iter_mut() {v.push(0);}
        for v in process_inner.mutex_need.iter_mut() {v.push(0);}
        process_inner.mutex_list.len() as isize - 1
    }
}
/// mutex lock syscall
pub fn sys_mutex_lock(mutex_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_mutex_lock",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    if process_inner.deadlock_detect {
        let tid = current_task().unwrap().inner_exclusive_access().res.as_ref().unwrap().tid;
        process_inner.mutex_need[tid][mutex_id] += 1;
        let task_count = process_inner.tasks.len();
        // let mutex_count = process_inner.mutex_list.len();
        let mut finish = vec![false; task_count];
        // let mut work = vec![0; mutex_count];
        let mut work = process_inner.mutex_available.clone();
        loop {
            let mut can_update = false;
            for (ti, needv) in process_inner.mutex_need.iter().enumerate() {
                if finish[ti] {continue;}
                let mut can_finish = true;
                for (mi, available) in work.iter().enumerate() {
                    if needv[mi] > *available {
                        can_finish = false;
                        break;
                    }
                }
                if can_finish {
                    can_update = true;
                    finish[ti] = true;
                    for (mi, available) in process_inner.mutex_allocation[ti].iter().enumerate() {
                        work[mi] += *available;
                    }
                }
            }
            if !can_update {break;}
        }
        for f in finish {
            if !f {
                return -0xDEAD;
            }
        }
    }
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    drop(process_inner);
    drop(process);
    mutex.lock();
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    if process_inner.deadlock_detect {
        let tid = current_task().unwrap().inner_exclusive_access().res.as_ref().unwrap().tid;
        process_inner.mutex_available[mutex_id] -= 1;
        process_inner.mutex_allocation[tid][mutex_id] += 1;
        process_inner.mutex_need[tid][mutex_id] -= 1;
    }
    0
}
/// mutex unlock syscall
pub fn sys_mutex_unlock(mutex_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_mutex_unlock",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    let tid = current_task().unwrap().inner_exclusive_access().res.as_ref().unwrap().tid;
    process_inner.mutex_available[mutex_id] += 1;
    process_inner.mutex_allocation[tid][mutex_id] -= 1;
    drop(process_inner);
    drop(process);
    mutex.unlock();
    0
}
/// semaphore create syscall
pub fn sys_semaphore_create(res_count: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_semaphore_create",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let id = if let Some(id) = process_inner
        .semaphore_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.semaphore_list[id] = Some(Arc::new(Semaphore::new(res_count)));
        process_inner.semaphore_available[id] = res_count;
        if process_inner.semaphore_need.is_empty() {
            let sem_count = process_inner.semaphore_list.len();
            process_inner.semaphore_allocation.push(vec![0; sem_count]);
            process_inner.semaphore_need.push(vec![0; sem_count]);
        }
        let _ = process_inner.semaphore_allocation.iter_mut().map(|v| v[id] = 0);
        let _ = process_inner.semaphore_need.iter_mut().map(|v| v[id] = 0);
        id
    } else {
        if process_inner.semaphore_need.is_empty() {
            let sem_count = process_inner.semaphore_list.len();
            process_inner.semaphore_allocation.push(vec![0; sem_count]);
            process_inner.semaphore_need.push(vec![0; sem_count]);
        }
        process_inner
            .semaphore_list
            .push(Some(Arc::new(Semaphore::new(res_count))));
        process_inner.semaphore_available.push(res_count);
        for v in process_inner.semaphore_allocation.iter_mut() {v.push(0);}
        for v in process_inner.semaphore_need.iter_mut() {v.push(0);}
        process_inner.semaphore_list.len() - 1
    };
    id as isize
}
/// semaphore up syscall
pub fn sys_semaphore_up(sem_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_semaphore_up",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let sem = Arc::clone(process_inner.semaphore_list[sem_id].as_ref().unwrap());
    let tid = current_task().unwrap().inner_exclusive_access().res.as_ref().unwrap().tid;
    process_inner.semaphore_available[sem_id] += 1;
    process_inner.semaphore_allocation[tid][sem_id] -= 1;
    drop(process_inner);
    sem.up();
    0
}
/// semaphore down syscall
pub fn sys_semaphore_down(sem_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_semaphore_down",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let sem = Arc::clone(process_inner.semaphore_list[sem_id].as_ref().unwrap());
    if process_inner.deadlock_detect {
        let tid = current_task().unwrap().inner_exclusive_access().res.as_ref().unwrap().tid;
        
        process_inner.semaphore_need[tid][sem_id] += 1;
        let task_count = process_inner.tasks.len();
        let mut finish = vec![false; task_count];
        let mut work = process_inner.semaphore_available.clone();
        loop {
            let mut can_update = false;
            for (ti, needv) in process_inner.semaphore_need.iter().enumerate() {
                if finish[ti] {continue;}
                let mut can_finish = true;
                for (mi, available) in work.iter().enumerate() {
                    if needv[mi] > *available {
                        can_finish = false;
                        break;
                    }
                }
                if can_finish {
                    can_update = true;
                    finish[ti] = true;
                    for (mi, available) in process_inner.semaphore_allocation[ti].iter().enumerate() {
                        work[mi] += *available;
                    }
                }
            }
            if !can_update {break;}
        }
        for f in finish {
            if !f {
                return -0xDEAD;
            }
        }
    }
    drop(process_inner);
    sem.down();
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    if process_inner.deadlock_detect {
        let tid = current_task().unwrap().inner_exclusive_access().res.as_ref().unwrap().tid;
        process_inner.semaphore_available[sem_id] -= 1;
        process_inner.semaphore_allocation[tid][sem_id] += 1;
        process_inner.semaphore_need[tid][sem_id] -= 1;
    }
    0
}
/// condvar create syscall
pub fn sys_condvar_create() -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_condvar_create",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let id = if let Some(id) = process_inner
        .condvar_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.condvar_list[id] = Some(Arc::new(Condvar::new()));
        id
    } else {
        process_inner
            .condvar_list
            .push(Some(Arc::new(Condvar::new())));
        process_inner.condvar_list.len() - 1
    };
    id as isize
}
/// condvar signal syscall
pub fn sys_condvar_signal(condvar_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_condvar_signal",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let condvar = Arc::clone(process_inner.condvar_list[condvar_id].as_ref().unwrap());
    drop(process_inner);
    condvar.signal();
    0
}
/// condvar wait syscall
pub fn sys_condvar_wait(condvar_id: usize, mutex_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_condvar_wait",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let condvar = Arc::clone(process_inner.condvar_list[condvar_id].as_ref().unwrap());
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    drop(process_inner);
    condvar.wait(mutex);
    0
}
/// enable deadlock detection syscall
///
/// YOUR JOB: Implement deadlock detection, but might not all in this syscall
pub fn sys_enable_deadlock_detect(enabled: usize) -> isize {
    trace!("kernel: sys_enable_deadlock_detect IMPLEMENTED");
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    match enabled {
        0 => {
            process_inner.deadlock_detect = false;
            0
        },
        1 => {
            process_inner.deadlock_detect = true;
            0
        },
        _ => -1
    }
}
