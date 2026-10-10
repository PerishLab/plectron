use super::descriptor;
use interprocess::os::windows::security_descriptor::AsSecurityDescriptorExt;
use std::os::windows::fs::MetadataExt;
use std::{fs::File, io, os::windows::io::FromRawHandle, path::Path};
use widestring::{U16CStr, U16CString};
use windows_sys::Win32::{
    Foundation::{GENERIC_READ, GENERIC_WRITE, INVALID_HANDLE_VALUE, LocalFree},
    Security::Authorization::{
        ConvertSecurityDescriptorToStringSecurityDescriptorW, GetNamedSecurityInfoW, SE_FILE_OBJECT,
    },
    Security::{DACL_SECURITY_INFORMATION, SECURITY_ATTRIBUTES},
    Storage::FileSystem::{
        CREATE_NEW, CreateDirectoryW, CreateFileW, FILE_ATTRIBUTE_NORMAL,
        FILE_FLAG_DELETE_ON_CLOSE, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
    },
};

pub fn prepare(path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let descriptor = descriptor()?;
    let name = U16CString::from_os_str(path).map_err(io::Error::other)?;
    let mut attributes: SECURITY_ATTRIBUTES = SECURITY_ATTRIBUTES {
        nLength: 0,
        lpSecurityDescriptor: std::ptr::null_mut(),
        bInheritHandle: 0,
    };
    attributes.nLength = size_of::<SECURITY_ATTRIBUTES>() as u32;
    descriptor.write_to_security_attributes(&mut attributes);
    // SAFETY: the name and descriptor remain live during creation; the attributes prohibit inheritance.
    if unsafe { CreateDirectoryW(name.as_ptr(), &attributes) } == 0 {
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::AlreadyExists {
            return Err(error);
        }
    }
    let metadata = std::fs::symlink_metadata(path)?;
    if !metadata.is_dir() || metadata.file_attributes() & 0x400 != 0 {
        return Err(io::Error::other("IPC storage is not an ordinary directory"));
    }
    let expected =
        descriptor.serialize(DACL_SECURITY_INFORMATION, |text| text.to_string_lossy())?;
    if permissions(&name)? != expected {
        return Err(io::Error::other(
            "IPC storage must grant access only to the current logon SID",
        ));
    }
    Ok(())
}

fn permissions(path: &U16CStr) -> io::Result<String> {
    let mut descriptor = std::ptr::null_mut();
    // SAFETY: path is NUL-terminated; unused outputs are null and the descriptor receives an owned local allocation.
    let result = unsafe {
        GetNamedSecurityInfoW(
            path.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut descriptor,
        )
    };
    if result != 0 {
        return Err(io::Error::from_raw_os_error(result as i32));
    }
    let mut text = std::ptr::null_mut();
    // SAFETY: the descriptor is initialized by GetNamedSecurityInfoW; conversion allocates a local string.
    let converted = unsafe {
        ConvertSecurityDescriptorToStringSecurityDescriptorW(
            descriptor,
            1,
            DACL_SECURITY_INFORMATION,
            &mut text,
            std::ptr::null_mut(),
        )
    };
    let failure = io::Error::last_os_error();
    // SAFETY: descriptor is the sole local allocation returned above and conversion no longer uses it.
    unsafe {
        LocalFree(descriptor);
    }
    if converted == 0 {
        return Err(failure);
    }
    // SAFETY: successful conversion returns a live NUL-terminated UTF-16 string.
    let value = unsafe { U16CStr::from_ptr_str(text) }.to_string_lossy();
    // SAFETY: text is the owned conversion output and has been copied.
    unsafe {
        LocalFree(text.cast());
    }
    Ok(value)
}

pub fn create(path: &Path, temporary: bool) -> io::Result<File> {
    let descriptor = descriptor()?;
    let name = U16CString::from_os_str(path).map_err(io::Error::other)?;
    let mut attributes: SECURITY_ATTRIBUTES = SECURITY_ATTRIBUTES {
        nLength: 0,
        lpSecurityDescriptor: std::ptr::null_mut(),
        bInheritHandle: 0,
    };
    attributes.nLength = size_of::<SECURITY_ATTRIBUTES>() as u32;
    descriptor.write_to_security_attributes(&mut attributes);
    let flags = FILE_ATTRIBUTE_NORMAL
        | if temporary {
            FILE_FLAG_DELETE_ON_CLOSE
        } else {
            0
        };
    // SAFETY: name and security descriptor are live; CREATE_NEW refuses existing targets; the returned handle is uniquely owned.
    let handle = unsafe {
        CreateFileW(
            name.as_ptr(),
            GENERIC_READ | GENERIC_WRITE,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            &attributes,
            CREATE_NEW,
            flags,
            std::ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: successful CreateFileW transfers one valid owned file handle.
    Ok(unsafe { File::from_raw_handle(handle) })
}
