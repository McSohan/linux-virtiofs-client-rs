use std::os::unix::net::UnixStream;
use std::io::{Read, Write};
use std::mem::{size_of};

#[repr(C)]
#[derive(Debug)]
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
#[derive(Debug)]
struct FuseInitIn {
    major: u32,
    minor: u32,
    max_readahead: u32,
    flags: u32,
}

#[repr(C)]
#[derive(Debug)]
struct FuseOutHeader {
    len: u32,
    error: i32,
    unique: u64,
}

#[repr(C)]
#[derive(Debug)]
struct FuseInitOut {
    major: u32,
    minor: u32,
    max_readahead: u32,
    flags: u32,
    max_background: u16,
    congestion_threshold: u16,
    max_write: u32,
    time_gran: u32,
    reserved: [u32; 9],
}

fn main() {
    let mut stream = UnixStream::connect("/tmp/vfs.sock")
        .expect("Failed to connect to virtiofsd");

    // ----- Build request -----
    let header = FuseInHeader {
        len: (size_of::<FuseInHeader>() + size_of::<FuseInitIn>()) as u32,
        opcode: 26, // FUSE_INIT
        unique: 1,
        nodeid: 0,
        uid: 0,
        gid: 0,
        pid: 0,
        padding: 0,
    };

    let init = FuseInitIn {
        major: 7,
        minor: 31,
        max_readahead: 0x20000,
        flags: 0,
    };

    let header_bytes = unsafe {
        std::slice::from_raw_parts(
            &header as *const _ as *const u8,
            size_of::<FuseInHeader>(),
        )
    };
    let init_bytes = unsafe {
        std::slice::from_raw_parts(
            &init as *const _ as *const u8,
            size_of::<FuseInitIn>(),
        )
    };

    let mut request = Vec::new();
    request.extend_from_slice(header_bytes);
    request.extend_from_slice(init_bytes);

    stream.write_all(&request).expect("send failed");
    println!("Sent FUSE_INIT ({}) bytes", request.len());

    // ----- Read reply -----
    let mut buf = vec![0u8; 1024];
    let n = stream.read(&mut buf).expect("read failed");
    println!("Received {} bytes", n);

    let out_header = unsafe {
        &*(buf.as_ptr() as *const FuseOutHeader)
    };
    println!("FUSE_OUT header: {:?}", out_header);

    let out_init = unsafe {
        &*(buf[size_of::<FuseOutHeader>()..].as_ptr() as *const FuseInitOut)
    };
    println!("FUSE_INIT_OUT: {:?}", out_init);
}
