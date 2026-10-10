use interprocess::os::windows::named_pipe::{PipeStream, pipe_mode::Bytes};
use std::{io, os::windows::io::AsRawHandle};
use windows_sys::Win32::{
    Foundation::{ERROR_BROKEN_PIPE, ERROR_PIPE_NOT_CONNECTED},
    System::Pipes::PeekNamedPipe,
};

pub fn available(pipe: &PipeStream<Bytes, Bytes>) -> io::Result<Option<u32>> {
    let mut count = 0;
    // SAFETY: the borrowed pipe handle stays open; only the initialized available-byte count is requested.
    if unsafe {
        PeekNamedPipe(
            pipe.as_raw_handle(),
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            &mut count,
            std::ptr::null_mut(),
        )
    } != 0
    {
        return Ok(Some(count));
    }
    let error = io::Error::last_os_error();
    if matches!(error.raw_os_error(), Some(code) if code == ERROR_BROKEN_PIPE as i32 || code == ERROR_PIPE_NOT_CONNECTED as i32)
    {
        return Ok(None);
    }
    Err(error)
}
