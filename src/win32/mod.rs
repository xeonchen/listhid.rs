extern crate winapi;

use std::ffi::OsStr;
use std::ffi::OsString;
use std::iter::once;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::prelude::*;
use std::{io, mem, ptr};

use winapi::shared::guiddef::GUID;
use winapi::shared::hidsdi::{
  HidD_GetAttributes, HidD_GetProductString, HidD_GetSerialNumberString, HIDD_ATTRIBUTES,
};
use winapi::shared::minwindef::DWORD;
use winapi::shared::ntdef::{FALSE, HANDLE, LPCWSTR, PCWSTR, PVOID, PWCHAR, WCHAR};
use winapi::shared::windef::HWND;
use winapi::shared::winerror::{ERROR_INSUFFICIENT_BUFFER, ERROR_NO_MORE_ITEMS};
use winapi::um::errhandlingapi::GetLastError;
use winapi::um::fileapi::CreateFileW;
use winapi::um::handleapi::{CloseHandle, INVALID_HANDLE_VALUE};
use winapi::um::setupapi::{
  SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInfo, SetupDiEnumDeviceInterfaces,
  SetupDiGetClassDevsW, SetupDiGetDeviceInterfaceDetailW, SetupDiGetDeviceRegistryPropertyW,
  HDEVINFO, PSP_DEVICE_INTERFACE_DETAIL_DATA_W, SPDRP_PHYSICAL_DEVICE_OBJECT_NAME,
  SP_DEVICE_INTERFACE_DATA, SP_DEVICE_INTERFACE_DETAIL_DATA_W, SP_DEVINFO_DATA,
};

pub struct HDevInfo {
  native_handle: Option<HDEVINFO>,
}

impl Drop for HDevInfo {
  fn drop(&mut self) {
    if let Some(h) = self.native_handle {
      unsafe { SetupDiDestroyDeviceInfoList(h) };
    }
  }
}

pub struct Handle {
  pub native_handle: Option<HANDLE>,
}

impl Drop for Handle {
  fn drop(&mut self) {
    if let Some(h) = self.native_handle {
      unsafe { CloseHandle(h) };
    }
  }
}

pub struct DeviceInterfaceDetail {
  pub device_path: String,
  pub device_info_data: SP_DEVINFO_DATA,
}

fn lpcwstr_to_string(wide_string: LPCWSTR, length: usize) -> String {
  let slice = unsafe {
    std::slice::from_raw_parts(wide_string, length)
      .split(|&v| v == 0)
      .next()
      .unwrap()
  };
  OsString::from_wide(slice).into_string().unwrap()
}

fn string_to_lpcwstr(s: &str) -> Vec<WCHAR> {
  OsStr::new(s).encode_wide().chain(once(0)).collect()
}

pub fn setup_di_get_class_devs(
  class_guid: *const GUID,
  enumerator: PCWSTR,
  hwnd_parent: HWND,
  flags: DWORD,
) -> Result<HDevInfo, io::Error> {
  match unsafe { SetupDiGetClassDevsW(class_guid, enumerator, hwnd_parent, flags) } {
    INVALID_HANDLE_VALUE => Err(io::Error::last_os_error()),
    handle => Ok(HDevInfo {
      native_handle: Some(handle),
    }),
  }
}

pub fn setup_di_enum_device_info(
  handle_dev_info: &HDevInfo,
) -> Result<std::vec::Vec<winapi::um::setupapi::SP_DEVINFO_DATA>, io::Error> {
  let mut device_info_entries = Vec::new();
  let mut index: u32 = 0;

  loop {
    let mut device_info_data: SP_DEVINFO_DATA = unsafe { mem::zeroed() };
    device_info_data.cbSize = mem::size_of::<SP_DEVINFO_DATA>() as u32;

    if unsafe {
      SetupDiEnumDeviceInfo(
        handle_dev_info.native_handle.unwrap_or(ptr::null_mut()),
        index,
        &mut device_info_data,
      )
    } == 0
    {
      match unsafe { GetLastError() } {
        ERROR_NO_MORE_ITEMS => break,
        _ => return Err(io::Error::last_os_error()),
      }
    }
    device_info_entries.push(device_info_data);
    index += 1;
  }

  Ok(device_info_entries)
}

pub fn setup_di_enum_device_interfaces(
  handle_dev_info: &HDevInfo,
  device_info_data: winapi::um::setupapi::PSP_DEVINFO_DATA,
  interface_class_guid: *const winapi::shared::guiddef::GUID,
) -> Result<Vec<winapi::um::setupapi::SP_DEVICE_INTERFACE_DATA>, io::Error> {
  let mut interface_data_entries = Vec::new();
  let mut index: u32 = 0;

  loop {
    let mut device_interface_data: SP_DEVICE_INTERFACE_DATA = unsafe { mem::zeroed() };
    device_interface_data.cbSize = mem::size_of::<SP_DEVICE_INTERFACE_DATA>() as u32;

    if unsafe {
      SetupDiEnumDeviceInterfaces(
        handle_dev_info.native_handle.unwrap_or(ptr::null_mut()),
        device_info_data,
        interface_class_guid,
        index,
        &mut device_interface_data,
      )
    } == 0
    {
      match unsafe { GetLastError() } {
        ERROR_NO_MORE_ITEMS => break,
        _ => return Err(io::Error::last_os_error()),
      }
    }
    interface_data_entries.push(device_interface_data);
    index += 1;
  }

  Ok(interface_data_entries)
}

pub fn setup_di_get_device_interface_detail(
  handle_dev_info: &HDevInfo,
  interface_data: winapi::um::setupapi::PSP_DEVICE_INTERFACE_DATA,
) -> Result<DeviceInterfaceDetail, io::Error> {
  let mut device_info_data: SP_DEVINFO_DATA = unsafe { mem::zeroed() };
  device_info_data.cbSize = mem::size_of::<SP_DEVINFO_DATA>() as u32;

  // 1. retrieve required size of the buffer
  let mut required_size: u32 = 0;
  if unsafe {
    SetupDiGetDeviceInterfaceDetailW(
      handle_dev_info.native_handle.unwrap_or(ptr::null_mut()),
      interface_data,
      ptr::null_mut(),
      0,
      &mut required_size,
      &mut device_info_data,
    )
  } == 0
    && unsafe { GetLastError() } != ERROR_INSUFFICIENT_BUFFER
  {
    return Err(io::Error::last_os_error());
  }

  // 2. prepare buffer
  let mut raw_memory = vec![0u8; required_size as usize];
  let device_interface_detail_data_ptr: PSP_DEVICE_INTERFACE_DETAIL_DATA_W =
    raw_memory.as_mut_ptr() as PSP_DEVICE_INTERFACE_DETAIL_DATA_W;
  let device_path_ptr: PWCHAR =
    unsafe { (*device_interface_detail_data_ptr).DevicePath.as_mut_ptr() };
  let path_size = (required_size as usize - mem::size_of::<DWORD>()) / mem::size_of::<WCHAR>();

  // 3. call the API again to retrieve the information
  if unsafe {
    (*device_interface_detail_data_ptr).cbSize =
      mem::size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>() as u32;

    SetupDiGetDeviceInterfaceDetailW(
      handle_dev_info.native_handle.unwrap_or(ptr::null_mut()),
      interface_data,
      device_interface_detail_data_ptr,
      raw_memory.len() as u32,
      &mut required_size,
      &mut device_info_data,
    )
  } == 0
  {
    return Err(io::Error::last_os_error());
  }

  Ok(DeviceInterfaceDetail {
    device_path: lpcwstr_to_string(device_path_ptr, path_size),
    device_info_data,
  })
}

pub fn create_file(
  file_name: &str,
  desired_access: DWORD,
  share_mode: DWORD,
  security_attributes: winapi::um::minwinbase::LPSECURITY_ATTRIBUTES,
  creation_disposition: DWORD,
  flags_and_attributes: DWORD,
  template_file: Handle,
) -> Result<Handle, io::Error> {
  match unsafe {
    CreateFileW(
      string_to_lpcwstr(file_name).as_ptr(),
      desired_access,
      share_mode,
      security_attributes,
      creation_disposition,
      flags_and_attributes,
      template_file.native_handle.unwrap_or(ptr::null_mut()),
    )
  } {
    INVALID_HANDLE_VALUE => Err(io::Error::last_os_error()),
    handle => Ok(Handle {
      native_handle: Some(handle),
    }),
  }
}

pub fn hid_d_get_attributes(
  handle: &Handle,
) -> Result<winapi::shared::hidsdi::HIDD_ATTRIBUTES, io::Error> {
  let mut attr: HIDD_ATTRIBUTES = unsafe { mem::zeroed() };

  if unsafe { HidD_GetAttributes(handle.native_handle.unwrap_or(ptr::null_mut()), &mut attr) } == 0
  {
    return Err(io::Error::last_os_error());
  }

  Ok(attr)
}

fn setup_di_get_device_registry_property(
  handle_dev_info: &HDevInfo,
  device_info_data: &mut winapi::um::setupapi::SP_DEVINFO_DATA,
  property: DWORD,
) -> Result<Vec<u8>, io::Error> {
  let mut property_reg_data_type: DWORD = 0;
  let mut required_size: DWORD = 0;
  unsafe {
    SetupDiGetDeviceRegistryPropertyW(
      handle_dev_info.native_handle.unwrap_or(ptr::null_mut()),
      device_info_data,
      property,
      &mut property_reg_data_type,
      ptr::null_mut(),
      0,
      &mut required_size,
    );
  };

  let mut raw_memory = vec![0u8; required_size as usize];
  unsafe {
    SetupDiGetDeviceRegistryPropertyW(
      handle_dev_info.native_handle.unwrap_or(ptr::null_mut()),
      device_info_data,
      property,
      &mut property_reg_data_type,
      raw_memory.as_mut_ptr(),
      raw_memory.len() as u32,
      ptr::null_mut(),
    );
  };

  Ok(raw_memory)
}

pub fn get_pdo_name(
  handle_dev_info: &HDevInfo,
  device_info_data: Option<winapi::um::setupapi::SP_DEVINFO_DATA>,
) -> Option<String> {
  let mut info_data = match device_info_data {
    None => return None,
    Some(data) => data,
  };

  let mut buffer = match setup_di_get_device_registry_property(
    &handle_dev_info,
    &mut info_data,
    SPDRP_PHYSICAL_DEVICE_OBJECT_NAME,
  ) {
    Err(_) => return None,
    Ok(b) => b,
  };

  let device_path_ptr: PWCHAR = buffer.as_mut_ptr() as PWCHAR;
  let device_path_size = (buffer.len()) / mem::size_of::<WCHAR>();
  Some(lpcwstr_to_string(device_path_ptr, device_path_size))
}

pub fn hid_d_get_product_string(handle: &Handle) -> Option<String> {
  unsafe {
    const MAXSIZE: usize = 127;
    let mut buffer: [WCHAR; MAXSIZE] = std::mem::zeroed();
    match HidD_GetProductString(
      handle.native_handle.unwrap_or(ptr::null_mut()),
      buffer.as_mut_ptr() as PVOID,
      buffer.len() as u32,
    ) {
      FALSE => None,
      _ => Some(lpcwstr_to_string(buffer.as_ptr(), buffer.len())),
    }
  }
}

pub fn hid_d_get_serial_number_string(handle: &Handle) -> Option<String> {
  unsafe {
    const MAXSIZE: usize = 127;
    let mut buffer: [WCHAR; MAXSIZE] = std::mem::zeroed();
    match HidD_GetSerialNumberString(
      handle.native_handle.unwrap_or(ptr::null_mut()),
      buffer.as_mut_ptr() as PVOID,
      buffer.len() as u32,
    ) {
      FALSE => None,
      _ => Some(lpcwstr_to_string(buffer.as_ptr(), buffer.len())),
    }
  }
}
