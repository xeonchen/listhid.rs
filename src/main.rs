use listhid::list_hid_device;

fn main() {
  match list_hid_device() {
    Ok(devices) => println!("hid devices: {:#?}", devices),
    Err(e) => println!("error: {}", e),
  }
}
