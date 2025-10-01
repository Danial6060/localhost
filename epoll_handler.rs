use std::io;
use std::os::unix::io::RawFd;

#[cfg(target_os = "linux")]
pub struct Epoll {
    epfd: RawFd,
}

#[cfg(target_os = "linux")]
impl Epoll {
    pub fn new() -> io::Result<Self> {
        let epfd = unsafe { libc::epoll_create1(0) };
        if epfd < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Epoll { epfd })
    }

    pub fn add(&self, fd: RawFd, events: u32, data: u64) -> io::Result<()> {
        let mut event = libc::epoll_event {
            events,
            u64: data,
        };

        let ret = unsafe {
            libc::epoll_ctl(self.epfd, libc::EPOLL_CTL_ADD, fd, &mut event)
        };

        if ret < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    pub fn modify(&self, fd: RawFd, events: u32, data: u64) -> io::Result<()> {
        let mut event = libc::epoll_event {
            events,
            u64: data,
        };

        let ret = unsafe {
            libc::epoll_ctl(self.epfd, libc::EPOLL_CTL_MOD, fd, &mut event)
        };

        if ret < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    pub fn delete(&self, fd: RawFd) -> io::Result<()> {
        let ret = unsafe {
            libc::epoll_ctl(self.epfd, libc::EPOLL_CTL_DEL, fd, std::ptr::null_mut())
        };

        if ret < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    pub fn wait(&self, events: &mut [libc::epoll_event], timeout: i32) -> io::Result<usize> {
        let ret = unsafe {
            libc::epoll_wait(
                self.epfd,
                events.as_mut_ptr(),
                events.len() as i32,
                timeout,
            )
        };

        if ret < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(ret as usize)
        }
    }
}

#[cfg(target_os = "linux")]
impl Drop for Epoll {
    fn drop(&mut self) {
        unsafe { libc::close(self.epfd) };
    }
}

// For macOS/BSD - use kqueue
#[cfg(not(target_os = "linux"))]
pub struct Epoll {
    kq: RawFd,
}

#[cfg(not(target_os = "linux"))]
impl Epoll {
    pub fn new() -> io::Result<Self> {
        let kq = unsafe { libc::kqueue() };
        if kq < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Epoll { kq })
    }

    pub fn add(&self, fd: RawFd, events: u32, data: u64) -> io::Result<()> {
        let mut kev = libc::kevent {
            ident: fd as usize,
            filter: if events & libc::EPOLLIN as u32 != 0 {
                libc::EVFILT_READ
            } else {
                libc::EVFILT_WRITE
            },
            flags: libc::EV_ADD | libc::EV_ENABLE,
            fflags: 0,
            data: 0,
            udata: data as *mut libc::c_void,
        };

        let ret = unsafe {
            libc::kevent(self.kq, &kev, 1, std::ptr::null_mut(), 0, std::ptr::null())
        };

        if ret < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    pub fn modify(&self, fd: RawFd, events: u32, data: u64) -> io::Result<()> {
        self.delete(fd)?;
        self.add(fd, events, data)
    }

    pub fn delete(&self, fd: RawFd) -> io::Result<()> {
        let mut kev = libc::kevent {
            ident: fd as usize,
            filter: libc::EVFILT_READ,
            flags: libc::EV_DELETE,
            fflags: 0,
            data: 0,
            udata: std::ptr::null_mut(),
        };

        unsafe {
            libc::kevent(self.kq, &kev, 1, std::ptr::null_mut(), 0, std::ptr::null());
        }
        Ok(())
    }

    pub fn wait(&self, events: &mut [libc::epoll_event], timeout: i32) -> io::Result<usize> {
        let mut kevents = vec![
            libc::kevent {
                ident: 0,
                filter: 0,
                flags: 0,
                fflags: 0,
                data: 0,
                udata: std::ptr::null_mut(),
            };
            events.len()
        ];

        let timeout_spec = if timeout < 0 {
            std::ptr::null()
        } else {
            &libc::timespec {
                tv_sec: (timeout / 1000) as libc::time_t,
                tv_nsec: ((timeout % 1000) * 1_000_000) as libc::c_long,
            }
        };

        let ret = unsafe {
            libc::kevent(
                self.kq,
                std::ptr::null(),
                0,
                kevents.as_mut_ptr(),
                kevents.len() as i32,
                timeout_spec,
            )
        };

        if ret < 0 {
            return Err(io::Error::last_os_error());
        }

        for i in 0..ret as usize {
            events[i].events = if kevents[i].filter == libc::EVFILT_READ {
                libc::EPOLLIN as u32
            } else {
                libc::EPOLLOUT as u32
            };
            events[i].u64 = kevents[i].udata as u64;
        }

        Ok(ret as usize)
    }
}

#[cfg(not(target_os = "linux"))]
impl Drop for Epoll {
    fn drop(&mut self) {
        unsafe { libc::close(self.kq) };
    }
}

pub fn set_nonblocking(fd: RawFd) -> io::Result<()> {
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL, 0) };
    if flags < 0 {
        return Err(io::Error::last_os_error());
    }

    let ret = unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) };
    if ret < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}