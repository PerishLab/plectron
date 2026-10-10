use interprocess::os::windows::security_descriptor::SecurityDescriptor;
use std::io;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use widestring::{U16CStr, U16CString};
use windows_sys::Win32::{
    Foundation::LocalFree,
    Security::Authorization::ConvertSidToStringSidW,
    Security::{GetTokenInformation, TOKEN_GROUPS, TOKEN_QUERY, TokenLogonSid},
    System::Threading::{GetCurrentProcess, OpenProcessToken},
};

pub fn descriptor() -> io::Result<SecurityDescriptor> {
    let sid = logon()?;
    let text = U16CString::from_str(format!("D:P(A;OICI;FA;;;{sid})")).map_err(io::Error::other)?;
    SecurityDescriptor::deserialize(&text)
}

fn logon() -> io::Result<String> {
    let token = token()?;
    let mut size = 0;
    // SAFETY: the token remains owned; the null output is the documented size query.
    unsafe {
        GetTokenInformation(
            token.as_raw_handle(),
            TokenLogonSid,
            std::ptr::null_mut(),
            0,
            &mut size,
        );
    }
    if size == 0 || size > 65_536 {
        return Err(io::Error::last_os_error());
    }
    let mut bytes = vec![0usize; (size as usize).div_ceil(size_of::<usize>())];
    // SAFETY: the word-aligned allocation has at least size writable bytes; the token remains live.
    let result = unsafe {
        GetTokenInformation(
            token.as_raw_handle(),
            TokenLogonSid,
            bytes.as_mut_ptr().cast(),
            size,
            &mut size,
        )
    };
    if result == 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: successful TokenLogonSid initializes TOKEN_GROUPS and its SID within the live allocation.
    let groups = unsafe { &*bytes.as_ptr().cast::<TOKEN_GROUPS>() };
    if groups.GroupCount != 1 {
        return Err(io::Error::other("process has no unique logon SID"));
    }
    let mut text = std::ptr::null_mut();
    // SAFETY: the SID belongs to the live token-information buffer; the API owns the returned local allocation.
    if unsafe { ConvertSidToStringSidW(groups.Groups[0].Sid, &mut text) } == 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: successful conversion returns a NUL-terminated UTF-16 string, copied before LocalFree.
    let sid = unsafe { U16CStr::from_ptr_str(text) }.to_string_lossy();
    // SAFETY: text is the allocation returned by ConvertSidToStringSidW and is no longer borrowed.
    unsafe {
        LocalFree(text.cast());
    }
    Ok(sid)
}

fn token() -> io::Result<OwnedHandle> {
    let mut handle = std::ptr::null_mut();
    // SAFETY: GetCurrentProcess is a borrowed pseudo-handle; output receives a newly owned query-only token.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut handle) } == 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: OpenProcessToken succeeded and transfers one valid token handle to this owner.
    Ok(unsafe { OwnedHandle::from_raw_handle(handle) })
}
