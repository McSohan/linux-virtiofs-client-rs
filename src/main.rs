use std::ffi::CString;
use std::fs::{create_dir_all, File};
use std::io::{Read, Write};
use std::mem::{size_of, zeroed};
use std::os::fd::FromRawFd;
use std::os::unix::io::{AsRawFd, RawFd};
use std::ptr;
use libc::*;

const FUSE_INIT: u32 = 26;
const FUSE_LOOKUP: u32 = 1;
const FUSE_GETATTR: u32 = 3;
const FUSE_READDIR: u32 = 28;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
struct FuseInHeader {
    len: u32,
    opcode: u32,
    unique: u64,
    nodeid: u64,
    uid: u32,
    gid: u32,
    pid: u32,
    padding: u32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
struct FuseOutHeader {
    len: u32,
    error: i32,
    unique: u64,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
struct FuseInitIn {
    major: u32,
    minor: u32,
    max_readahead: u32,
    flags: u32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
struct FuseInitOut {
    major: u32,
    minor: u32,
    max_readahead: u32,
    flags: u32,
    max_background: u32,
    congestion_threshold: u32,
    max_write: u32,
    time_gran: u32,
    unused: [u32; 9],
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
struct FuseAttrOut {
    attr_valid: u64,
    attr_valid_nsec: u32,
    dummy: u32,
    st: stat,
}

unsafe fn mount_fuse(fd: RawFd, mountpoint: &str) {
    let opts = format!("fd={fd},rootmode=40000,user_id=0,group_id=0");
    let src = CString::new("fuse").unwrap();
    let typ = CString::new("fuse").unwrap();
    let tgt = CString::new(mountpoint).unwrap();
    let data = CString::new(opts).unwrap();

    let ret = mount(
        src.as_ptr(),
        tgt.as_ptr(),
        typ.as_ptr(),
        (MS_NOSUID | MS_NODEV) as c_ulong,
        data.as_ptr() as *const libc::c_void,
    );
    if ret != 0 {
        panic!("mount() failed: {}", std::io::Error::last_os_error());
    }
}

fn main() {
    let mp = "/tmp/mnt";

    // 1. Open /dev/fuse
    let file = File::open("/dev/fuse").unwrap();
    let fd = file.as_raw_fd();
    println!("[+] fuse fd = {}", fd);

    unsafe { mount_fuse(fd, mp); }
    println!("[+] Mounted at {}", mp);

    let mut buf = [0u8; 8192];

    // 2. Read INIT request
    let n = unsafe { read(fd, buf.as_mut_ptr() as *mut _, buf.len()) };
    if n <= 0 {
        panic!("read INIT failed");
    }

    let inh: &FuseInHeader = unsafe { &*(buf.as_ptr() as *const FuseInHeader) };
    println!("[+] Got INIT: opcode={}, unique={}", inh.opcode, inh.unique);

    if inh.opcode != FUSE_INIT {
        panic!("Expected FUSE_INIT");
    }

    let init_in: &FuseInitIn = unsafe {
        &*(buf[std::mem::size_of::<FuseInHeader>()..].as_ptr() as *const FuseInitIn)
    };

    println!(
        "[+] Kernel wants version {}.{}",
        init_in.major, init_in.minor
    );

    // 3. Send INIT response
    let mut out_hdr = FuseOutHeader {
        len: (size_of::<FuseOutHeader>() + size_of::<FuseInitOut>()) as u32,
        error: 0,
        unique: inh.unique,
    };

    let mut init_out: FuseInitOut = unsafe { zeroed() };
    init_out.major = 7;
    init_out.minor = 31;
    init_out.max_readahead = init_in.max_readahead;
    init_out.max_write = 131072;
    init_out.time_gran = 1;

    unsafe {
        write(fd, &out_hdr as *const _ as *const _, size_of::<FuseOutHeader>());
        write(fd, &init_out as *const _ as *const _, size_of::<FuseInitOut>());
    }

    println!("[+] INIT done, entering main loop");

    // 4. Main loop
    loop {
        let n = unsafe { read(fd, buf.as_mut_ptr() as *mut _, buf.len()) };
        if n <= 0 {
            panic!("read failed");
        }

        let inh: &FuseInHeader = unsafe { &*(buf.as_ptr() as *const FuseInHeader) };
        println!("[+] Got request: opcode={}, nodeid={}", inh.opcode, inh.nodeid);

        match inh.opcode {
            FUSE_GETATTR => unsafe {
                let mut reply: FuseAttrOut = zeroed();
                reply.attr_valid = 1;
                reply.st.st_mode = S_IFDIR | 0o755;
                reply.st.st_nlink = 2;

                let oh = FuseOutHeader {
                    len: (size_of::<FuseOutHeader>() + size_of::<FuseAttrOut>()) as u32,
                    error: 0,
                    unique: inh.unique,
                };

                write(fd, &oh as *const _ as *const _, size_of::<FuseOutHeader>());
                write(fd, &reply as *const _ as *const _, size_of::<FuseAttrOut>());
            },

            FUSE_LOOKUP => unsafe {
                // No files → ENOENT
                let oh = FuseOutHeader {
                    len: size_of::<FuseOutHeader>() as u32,
                    error: -(ENOENT as i32),
                    unique: inh.unique,
                };
                write(fd, &oh as *const _ as *const _, size_of::<FuseOutHeader>());
            },

            FUSE_READDIR => unsafe {
                // Empty directory listing
                let oh = FuseOutHeader {
                    len: size_of::<FuseOutHeader>() as u32,
                    error: 0,
                    unique: inh.unique,
                };
                write(fd, &oh as *const _ as *const _, size_of::<FuseOutHeader>());
            },

            _ => unsafe {
                let oh = FuseOutHeader {
                    len: size_of::<FuseOutHeader>() as u32,
                    error: -(ENOSYS as i32),
                    unique: inh.unique,
                };
                write(fd, &oh as *const _ as *const _, size_of::<FuseOutHeader>());
            },
        }
    }
}
