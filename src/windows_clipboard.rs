// Copyright 2016 Avraham Weinstock
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use clipboard_win::{get_clipboard_string, set_clipboard_string, Clipboard};
use clipboard_win::raw::{get_clipboard_data, register_format};
use clipboard_win::utils::LockedData;
use std::os::raw::c_int;
use crate::common::{ClipboardProvider, Result};

pub struct WindowsClipboardContext;

impl WindowsClipboardContext {
    pub fn new() -> Result<Self> {
        Ok(WindowsClipboardContext)
    }
}

impl ClipboardProvider for WindowsClipboardContext {
    fn get_contents(&mut self) -> Result<String> {
        Ok(get_clipboard_string()?)
    }

    fn set_contents(&mut self, data: String) -> Result<()> {
        Ok(set_clipboard_string(&data)?)
    }

    fn get_mime_contents(&mut self, mime: &str) -> Result<String> {
        let format = register_format(mime.into())?;
        let mut data = String::new();
        let mut clipboard = Clipboard::new_attempts(10)?;
        let _ = get_data_at_format(&mut clipboard,&mut data, format)?;
        Ok(data)
    }
    fn set_mime_contents(&mut self, data: String, mime: &str) -> Result<()> {
        let format = register_format(mime.into())?;
        let mut clipboard = Clipboard::new_attempts(10)?;
        let _ = set_data_at_format(&mut clipboard, &data, format)?;
        Ok(())
    }
}

// Copied from "clipboard-win" crate and modified to allow specifying a format parameter.
// Note that we pass in the Clipboard as first parameter to keep the clipboard open.
// TODO: Move this functionality back into the clipboard-win crate - for now we want to minimize the amount of forks we have.
fn set_data_at_format(_: &mut Clipboard, data: &str, format: u32) -> io::Result<()> {
    use winapi::shared::basetsd::*;
    use winapi::um::winbase::*;
    use winapi::um::stringapiset::MultiByteToWideChar;
    use winapi::um::winnls::CP_UTF8;
    use winapi::um::winuser::*;

    debug_assert!(data.len() > 0);

    let size = unsafe {
        MultiByteToWideChar(CP_UTF8, 0, data.as_ptr() as *const _, data.len() as c_int, std::ptr::null_mut(), 0)
    };

    if size == 0 {
        return Err(std::io::Error::last_os_error())
    }

    let alloc_handle = unsafe { GlobalAlloc(GHND, mem::size_of::<u16>() * (size as SIZE_T + 1)) };

    if alloc_handle.is_null() {
        Err(std::io::Error::last_os_error())
    }
    else {
        unsafe {
            {
                let (ptr, _lock) = LockedData::new(alloc_handle)?;
                MultiByteToWideChar(CP_UTF8, 0, data.as_ptr() as *const _, data.len() as c_int, ptr.as_ptr(), size);
                std::ptr::write(ptr.as_ptr().offset(size as isize), 0);
            }
            EmptyClipboard();

            if SetClipboardData(format, alloc_handle).is_null() {
                let result = std::io::Error::last_os_error();
                GlobalFree(alloc_handle);
                Err(result)
            }
            else {
                Ok(())
            }
        }
    }
}

// Copied from "clipboard-win" crate and modified to allow specifying a format parameter.
// Note that we pass in the Clipboard as first parameter to keep the clipboard open.
// TODO: Move this functionality back into the clipboard-win crate - for now we want to minimize the amount of forks we have.
fn get_data_at_format(_: &mut Clipboard, storage: &mut String, format: u32) -> std::io::Result<()> {
    use winapi::um::stringapiset::WideCharToMultiByte;
    use winapi::um::winnls::CP_UTF8;
    use winapi::um::winbase::*;

    let clipboard_data = get_clipboard_data(format)?;

    unsafe {
        let (data_ptr, _guard) = LockedData::new(clipboard_data.as_ptr())?;

        let data_size = GlobalSize(clipboard_data.as_ptr()) as usize / std::mem::size_of::<u16>();
        let storage_req_size = WideCharToMultiByte(CP_UTF8, 0, data_ptr.as_ptr(), data_size as c_int, std::ptr::null_mut(), 0, std::ptr::null(), std::ptr::null_mut());
        if storage_req_size == 0 {
            return Err(std::io::Error::last_os_error());
        }

        {
            storage.reserve(storage_req_size as usize);
            let storage = storage.as_mut_vec();
            let storage_cursor = storage.len();
            let storage_ptr = storage.as_mut_ptr().add(storage_cursor) as *mut _;
            WideCharToMultiByte(CP_UTF8, 0, data_ptr.as_ptr(), data_size as c_int, storage_ptr, storage_req_size, std::ptr::null(), std::ptr::null_mut());
            storage.set_len(storage_cursor + storage_req_size as usize);
        }

        //It seems WinAPI always supposed to have at the end null char.
        //But just to be safe let's check for it and only then remove.
        if let Some(null_idx) = storage.find('\0') {
            storage.drain(null_idx..);
        }

        Ok(())
    }
}
