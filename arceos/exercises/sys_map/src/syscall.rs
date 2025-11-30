#![allow(dead_code)]

use core::ffi::{c_void, c_char, c_int};
use axhal::arch::TrapFrame;
use axhal::trap::{register_trap_handler, SYSCALL};
use axerrno::{LinuxError, LinuxResult};
use axtask::current;
use axtask::TaskExtRef;
use axhal::paging::MappingFlags;
use memory_addr::{VirtAddr, VirtAddrRange, PAGE_SIZE_4K};
use alloc::vec::Vec;
use arceos_posix_api as api;

const SYS_IOCTL: usize = 29;
const SYS_OPENAT: usize = 56;
const SYS_CLOSE: usize = 57;
const SYS_READ: usize = 63;
const SYS_WRITE: usize = 64;
const SYS_WRITEV: usize = 66;
const SYS_EXIT: usize = 93;
const SYS_EXIT_GROUP: usize = 94;
const SYS_SET_TID_ADDRESS: usize = 96;
const SYS_MMAP: usize = 222;

const AT_FDCWD: i32 = -100;

/// Macro to generate syscall body
///
/// It will receive a function which return Result<_, LinuxError> and convert it to
/// the type which is specified by the caller.
#[macro_export]
macro_rules! syscall_body {
    ($fn: ident, $($stmt: tt)*) => {{
        #[allow(clippy::redundant_closure_call)]
        let res = (|| -> axerrno::LinuxResult<_> { $($stmt)* })();
        match res {
            Ok(_) | Err(axerrno::LinuxError::EAGAIN) => debug!(concat!(stringify!($fn), " => {:?}"),  res),
            Err(_) => info!(concat!(stringify!($fn), " => {:?}"), res),
        }
        match res {
            Ok(v) => v as _,
            Err(e) => {
                -e.code() as _
            }
        }
    }};
}

bitflags::bitflags! {
    #[derive(Debug)]
    /// permissions for sys_mmap
    ///
    /// See <https://github.com/bminor/glibc/blob/master/bits/mman.h>
    struct MmapProt: i32 {
        /// Page can be read.
        const PROT_READ = 1 << 0;
        /// Page can be written.
        const PROT_WRITE = 1 << 1;
        /// Page can be executed.
        const PROT_EXEC = 1 << 2;
    }
}

impl From<MmapProt> for MappingFlags {
    fn from(value: MmapProt) -> Self {
        let mut flags = MappingFlags::USER;
        if value.contains(MmapProt::PROT_READ) {
            flags |= MappingFlags::READ;
        }
        if value.contains(MmapProt::PROT_WRITE) {
            flags |= MappingFlags::WRITE;
        }
        if value.contains(MmapProt::PROT_EXEC) {
            flags |= MappingFlags::EXECUTE;
        }
        flags
    }
}

bitflags::bitflags! {
    #[derive(Debug)]
    /// flags for sys_mmap
    ///
    /// See <https://github.com/bminor/glibc/blob/master/bits/mman.h>
    struct MmapFlags: i32 {
        /// Share changes
        const MAP_SHARED = 1 << 0;
        /// Changes private; copy pages on write.
        const MAP_PRIVATE = 1 << 1;
        /// Map address must be exactly as requested, no matter whether it is available.
        const MAP_FIXED = 1 << 4;
        /// Don't use a file.
        const MAP_ANONYMOUS = 1 << 5;
        /// Don't check for reservations.
        const MAP_NORESERVE = 1 << 14;
        /// Allocation is for a stack.
        const MAP_STACK = 0x20000;
    }
}

#[register_trap_handler(SYSCALL)]
fn handle_syscall(tf: &TrapFrame, syscall_num: usize) -> isize {
    ax_println!("handle_syscall [{}] ...", syscall_num);
    let ret = match syscall_num {
         SYS_IOCTL => sys_ioctl(tf.arg0() as _, tf.arg1() as _, tf.arg2() as _) as _,
        SYS_SET_TID_ADDRESS => sys_set_tid_address(tf.arg0() as _),
        SYS_OPENAT => sys_openat(tf.arg0() as _, tf.arg1() as _, tf.arg2() as _, tf.arg3() as _),
        SYS_CLOSE => sys_close(tf.arg0() as _),
        SYS_READ => sys_read(tf.arg0() as _, tf.arg1() as _, tf.arg2() as _),
        SYS_WRITE => sys_write(tf.arg0() as _, tf.arg1() as _, tf.arg2() as _),
        SYS_WRITEV => sys_writev(tf.arg0() as _, tf.arg1() as _, tf.arg2() as _),
        SYS_EXIT_GROUP => {
            ax_println!("[SYS_EXIT_GROUP]: system is exiting ..");
            axtask::exit(tf.arg0() as _)
        },
        SYS_EXIT => {
            ax_println!("[SYS_EXIT]: system is exiting ..");
            axtask::exit(tf.arg0() as _)
        },
        SYS_MMAP => sys_mmap(
            tf.arg0() as _,
            tf.arg1() as _,
            tf.arg2() as _,
            tf.arg3() as _,
            tf.arg4() as _,
            tf.arg5() as _,
        ),
        _ => {
            ax_println!("Unimplemented syscall: {}", syscall_num);
            -LinuxError::ENOSYS.code() as _
        }
    };
    ret
}

fn sys_mmap(
    addr: usize,
    length: usize,
    prot: i32,
    flags: i32,
    fd: i32,
    offset: isize,
) -> isize {
    syscall_body!(sys_mmap, {
        // 解析 flags 和 prot
        let mmap_flags = MmapFlags::from_bits(flags)
            .ok_or(LinuxError::EINVAL)?;
        let mmap_prot = MmapProt::from_bits(prot)
            .ok_or(LinuxError::EINVAL)?;
        
        // 对齐长度到 4KB
        let aligned_length = (length + PAGE_SIZE_4K - 1) & !(PAGE_SIZE_4K - 1);
        if aligned_length == 0 {
            return Err(LinuxError::EINVAL);
        }
        
        // 获取地址空间
        let curr = current();
        let mut aspace = curr.task_ext().aspace.lock();
        
        // 确定映射地址
        let start_addr = if mmap_flags.contains(MmapFlags::MAP_FIXED) {
            // MAP_FIXED: 使用指定地址（需要对齐）
            let vaddr = VirtAddr::from(addr);
            if !vaddr.is_aligned_4k() {
                return Err(LinuxError::EINVAL);
            }
            vaddr
        } else {
            // 查找空闲区域
            let hint = VirtAddr::from(addr);
            let limit = VirtAddrRange::from_start_size(
                aspace.base(),
                aspace.size()
            );
            aspace.find_free_area(hint, aligned_length, limit)
                .ok_or(LinuxError::ENOMEM)?
        };
        
        // 转换权限标志
        let mapping_flags = MappingFlags::from(mmap_prot);
        
        // 处理文件映射或匿名映射
        if mmap_flags.contains(MmapFlags::MAP_ANONYMOUS) {
            // 匿名映射：直接分配内存
            aspace.map_alloc(start_addr, aligned_length, mapping_flags, true)
                .map_err(|e| match e {
                    axerrno::AxError::NoMemory => LinuxError::ENOMEM,
                    axerrno::AxError::InvalidInput => LinuxError::EINVAL,
                    _ => LinuxError::EAGAIN,
                })?;
        } else {
            // 文件映射：需要从文件读取内容
            if fd < 0 {
                return Err(LinuxError::EBADF);
            }
            
            // 获取文件对象
            let file_like = api::imp::fd_ops::get_file_like(fd)?;
            
            // 分配内存
            aspace.map_alloc(start_addr, aligned_length, mapping_flags, true)
                .map_err(|e| match e {
                    axerrno::AxError::NoMemory => LinuxError::ENOMEM,
                    axerrno::AxError::InvalidInput => LinuxError::EINVAL,
                    _ => LinuxError::EAGAIN,
                })?;
            
            // 读取文件内容到临时缓冲区
            let mut file_data = vec![0u8; length];
            let mut total_read = 0;
            
            // 如果 offset 不为 0，需要先 seek 到 offset 位置
            // 保存当前位置（通过 sys_lseek 获取）
            let saved_pos = if offset != 0 {
                // 获取当前位置
                let current_pos = api::sys_lseek(fd, 0, 1); // SEEK_CUR = 1
                // Seek 到 offset
                let _ = api::sys_lseek(fd, offset, 0); // SEEK_SET = 0
                Some(current_pos)
            } else {
                None
            };
            
            // 读取文件内容
            while total_read < length {
                let buf = &mut file_data[total_read..];
                let read_size = file_like.read(buf)?;
                if read_size == 0 {
                    break; // EOF
                }
                total_read += read_size;
            }
            
            // 恢复文件位置（如果之前保存了）
            if let Some(pos) = saved_pos {
                let _ = api::sys_lseek(fd, pos, 0); // SEEK_SET = 0
            }
            
            // 将文件内容写入映射的内存
            aspace.write(start_addr, &file_data[..total_read])
                .map_err(|_| LinuxError::EFAULT)?;
        }
        
        Ok(start_addr.as_usize() as isize)
    })
}

fn sys_openat(dfd: c_int, fname: *const c_char, flags: c_int, mode: api::ctypes::mode_t) -> isize {
    assert_eq!(dfd, AT_FDCWD);
    api::sys_open(fname, flags, mode) as isize
}

fn sys_close(fd: i32) -> isize {
    api::sys_close(fd) as isize
}

fn sys_read(fd: i32, buf: *mut c_void, count: usize) -> isize {
    api::sys_read(fd, buf, count)
}

fn sys_write(fd: i32, buf: *const c_void, count: usize) -> isize {
    api::sys_write(fd, buf, count)
}

fn sys_writev(fd: i32, iov: *const api::ctypes::iovec, iocnt: i32) -> isize {
    unsafe { api::sys_writev(fd, iov, iocnt) }
}

fn sys_set_tid_address(tid_ptd: *const i32) -> isize {
    let curr = current();
    curr.task_ext().set_clear_child_tid(tid_ptd as _);
    curr.id().as_u64() as isize
}

fn sys_ioctl(_fd: i32, _op: usize, _argp: *mut c_void) -> i32 {
    ax_println!("Ignore SYS_IOCTL");
    0
}
