use crate::{errors::IOResult, riscv64::syscall, BasicKernelInterface, FileDescriptor};

/// Concrete implementation of the [`KernelIO`] trait for the `riscv64` target architecture.
#[derive(Debug)]
pub(crate) struct RiscV64IO;

/// Relevant system call numbers for the `riscv64` target architecture.
///
/// See https://jborza.com/post/2021-05-11-riscv-linux-syscalls/
///
/// **Note**: This is not an exhaustive list of system calls available to the `client` program,
/// only the ones necessary for the [BasicKernelInterface] trait implementation. If an extension
/// trait for the [BasicKernelInterface] trait is created for the linux kernel, this list
/// should be extended accordingly.
#[repr(usize)]
pub(crate) enum SyscallNumber {
    /// Sets the Exited and ExitCode states to true and $a0 respectively.
    Exit = 93,
    /// Similar behavior as Linux with support for unaligned reads.
    Read = 63,
    /// Similar behavior as Linux with support for unaligned writes.
    Write = 64,
}

impl BasicKernelInterface for RiscV64IO {
    fn write(fd: FileDescriptor, buf: &[u8]) -> IOResult<usize> {
        unsafe {
            crate::linux::from_ret(syscall::syscall3(
                SyscallNumber::Write as usize,
                fd.into(),
                buf.as_ptr() as usize,
                buf.len(),
            ))
        }
    }

    fn read(fd: FileDescriptor, buf: &mut [u8]) -> IOResult<usize> {
        unsafe {
            crate::linux::from_ret(syscall::syscall3(
                SyscallNumber::Read as usize,
                fd.into(),
                buf.as_ptr() as usize,
                buf.len(),
            ))
        }
    }

    fn exit(code: usize) -> ! {
        unsafe {
            let _ = syscall::syscall1(SyscallNumber::Exit as usize, code);
            panic!()
        }
    }
}
