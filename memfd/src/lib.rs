use std::{
    ffi::CStr,
    io::{Error as IoError, Result as IoResult},
    os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, OwnedFd},
};

const MEMFD_NAME: &CStr = c"progmemfd";

#[derive(Debug)]
pub struct Shm {
    fd: OwnedFd,
    ptr: *mut u8,
    length: usize,
}

impl Shm {
    pub fn new(length: usize) -> IoResult<Self> {
        if length == 0 {
            return Err(IoError::new(
                std::io::ErrorKind::InvalidInput,
                "Zero-length SHM is not allowed",
            ));
        }

        let fd = allocate_shm(length)?;
        let ptr = map_shm_memory(length, fd.as_fd())?;
        Ok(Shm { fd, ptr, length })
    }

    pub fn resize(&mut self, length: usize) -> IoResult<()> {
        match remap_shm_memory(length, self.fd.as_fd()) {
            Ok(ptr) => {
                self.ptr = ptr;
                self.length = length;
                Ok(())
            }
            Err(err) => Err(err),
        }
    }

    pub fn get_fd<'a>(&'a self) -> BorrowedFd<'a> {
        self.fd.as_fd()
    }

    pub fn get_raw_fd(&self) -> i32 {
        self.fd.as_raw_fd()
    }

    pub fn data(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.ptr, self.length) }
    }

    pub fn data_mut(&mut self) -> &mut [u8] {
        unsafe { std::slice::from_raw_parts_mut(self.ptr, self.length) }
    }
}

/// Allocates memory for shared processes, returning the file descriptor pointing to the allocated
/// memory
fn allocate_shm(length: usize) -> IoResult<OwnedFd> {
    unsafe {
        let fd = match libc::memfd_create(MEMFD_NAME.as_ptr(), libc::MFD_CLOEXEC) {
            -1 => Err(IoError::last_os_error()),
            fd => Ok(OwnedFd::from_raw_fd(fd)),
        }?;

        match libc::ftruncate(fd.as_raw_fd(), length as libc::off_t) {
            -1 => Err(IoError::last_os_error()),
            _ => Ok(fd),
        }
    }
}

/// Resize an already mapped a shm buffer
fn remap_shm_memory(length: usize, fd: BorrowedFd) -> IoResult<*mut u8> {
    if unsafe { libc::ftruncate(fd.as_raw_fd(), length as libc::off_t) } == -1 {
        return Err(IoError::last_os_error());
    }
    map_shm_memory(length, fd)
}

/// Maps memory to a shm buffer, returning the newly mapped byte array
fn map_shm_memory(length: usize, fd: BorrowedFd) -> IoResult<*mut u8> {
    unsafe {
        let ptr = libc::mmap(
            std::ptr::null_mut(),
            length,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_SHARED,
            fd.as_raw_fd(),
            0,
        ) as *mut u8;

        if ptr.addr() as isize == -1 {
            return Err(IoError::last_os_error());
        }

        Ok(ptr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invoke_single() {
        Shm::new(1024).expect("Failed simple");
    }

    #[test]
    fn invoke_a_lot() {
        for i in 0..500 {
            Shm::new(1024).expect(&format!("Failed at {i}"));
        }
    }

    #[test]
    #[should_panic]
    fn alloc_zero() {
        Shm::new(0).expect("Zero byte allocations should be fail");
    }

    #[test]
    fn alloc_4kb() {
        Shm::new(4 * 1024).expect("Failed 4kb");
    }

    #[test]
    fn alloc_2mb() {
        Shm::new(2 * 1024 * 1024).expect("Failed 2mb");
    }

    #[test]
    fn alloc_gb() {
        Shm::new(1024 * 1024 * 1024).expect("Failed 1gb");
    }
}
