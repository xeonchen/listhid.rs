#[cfg(windows)]
mod win32;

#[derive(Debug)]
pub struct HidDevice {
  pub path: String,
  pub product_id: u16,
  pub vendor_id: u16,
  pub product_string: Option<String>,
  pub serial_number_string: Option<String>,
  pub dev_inst: Option<u32>,
  pub pdo_name: Option<String>,
}

#[cfg(windows)]
struct DeviceData {
  interface_data: winapi::um::setupapi::SP_DEVICE_INTERFACE_DATA,
  info_data: Option<winapi::um::setupapi::SP_DEVINFO_DATA>,
}

#[cfg(windows)]
fn build_device_data_with_info(
  class_devs_info: &win32::HDevInfo,
  device_info_data_entries: std::vec::Vec<winapi::um::setupapi::SP_DEVINFO_DATA>,
) -> Result<std::vec::Vec<DeviceData>, std::io::Error> {
  use win32::setup_di_enum_device_interfaces;
  use winapi::shared::hidclass::GUID_DEVINTERFACE_HID;

  let mut devices = Vec::new();

  for mut device_info_data in device_info_data_entries {
    let interface_data_entries = setup_di_enum_device_interfaces(
      &class_devs_info,
      &mut device_info_data,
      &GUID_DEVINTERFACE_HID,
    )?;
    for interface_data in interface_data_entries {
      devices.push(DeviceData {
        interface_data,
        info_data: Some(device_info_data),
      });
    }
  }

  Ok(devices)
}

#[cfg(windows)]
fn build_device_data_without_info(
  class_devs_info: &win32::HDevInfo,
) -> Result<std::vec::Vec<DeviceData>, std::io::Error> {
  use win32::setup_di_enum_device_interfaces;
  use winapi::shared::hidclass::GUID_DEVINTERFACE_HID;

  let mut devices = Vec::new();
  let interface_data_entries = setup_di_enum_device_interfaces(
    &class_devs_info,
    std::ptr::null_mut(),
    &GUID_DEVINTERFACE_HID,
  )?;
  for interface_data in interface_data_entries {
    devices.push(DeviceData {
      interface_data,
      info_data: None,
    });
  }

  Ok(devices)
}

#[cfg(windows)]
fn build_device_data(
  class_devs_info: &win32::HDevInfo,
) -> Result<std::vec::Vec<DeviceData>, std::io::Error> {
  use win32::setup_di_enum_device_info;

  match setup_di_enum_device_info(&class_devs_info) {
    Ok(device_info_data_entries) => {
      build_device_data_with_info(class_devs_info, device_info_data_entries)
    }
    Err(_) => build_device_data_without_info(class_devs_info),
  }
}

#[cfg(not(windows))]
pub fn list_hid_device() -> Result<(), &'static str> {
  Err("unsupported platform")
}

#[cfg(windows)]
pub fn list_hid_device() -> Result<Vec<HidDevice>, std::io::Error> {
  use std::ptr;
  use win32::{
    create_file, get_pdo_name, hid_d_get_attributes, hid_d_get_product_string,
    hid_d_get_serial_number_string, setup_di_get_class_devs, setup_di_get_device_interface_detail,
    Handle,
  };
  use winapi::um::fileapi::OPEN_EXISTING;
  use winapi::um::setupapi::{DIGCF_ALLCLASSES, DIGCF_DEVICEINTERFACE, DIGCF_PRESENT};
  use winapi::um::winnt::{FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, FILE_SHARE_WRITE};

  let class_devs_info = setup_di_get_class_devs(
    ptr::null(),
    ptr::null(),
    ptr::null_mut(),
    DIGCF_ALLCLASSES | DIGCF_PRESENT | DIGCF_DEVICEINTERFACE,
  )?;

  let mut devices = Vec::new();

  for mut device_data in build_device_data(&class_devs_info)? {
    let device_interface_detail =
      setup_di_get_device_interface_detail(&class_devs_info, &mut device_data.interface_data)?;

    let handle = create_file(
      &device_interface_detail.device_path,
      0,
      FILE_SHARE_READ | FILE_SHARE_WRITE,
      ptr::null_mut(),
      OPEN_EXISTING,
      FILE_ATTRIBUTE_NORMAL,
      Handle {
        native_handle: None,
      },
    )?;

    let hidd_attributes = hid_d_get_attributes(&handle)?;

    devices.push(HidDevice {
      path: device_interface_detail.device_path,
      product_id: hidd_attributes.ProductID,
      vendor_id: hidd_attributes.VendorID,
      product_string: hid_d_get_product_string(&handle),
      serial_number_string: hid_d_get_serial_number_string(&handle),
      dev_inst: Some(device_interface_detail.device_info_data.DevInst),
      pdo_name: get_pdo_name(&class_devs_info, device_data.info_data),
    });
  }

  Ok(devices)
}
