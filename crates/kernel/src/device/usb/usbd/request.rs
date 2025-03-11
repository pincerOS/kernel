/******************************************************************************
*	usbd/devicerequest.h
*	 by Alex Chadwick
*
*	A light weight implementation of the USB protocol stack fit for a simple
*	driver.
*
*   Converted to Rust by Aaron Lo
*
*	usbd/devicerequest.h contains a definition of the standard device
*	request structure defined in USB2.0
******************************************************************************/

/// An encapsulated device request.
///
/// A device request is a standard mechanism defined in USB2.0 manual section
/// 9.3 by which negotiations with devices occur. The request has a number of
/// parameters, and so are best implemented with a structure. As per usual,
/// since this structure is arbitrary, we shall match Linux in the hopes of
/// achieving some compatibility.
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct UsbDeviceRequest {
    pub request_type: u8, // +0x0

    pub request: UsbDeviceRequestRequest, // +0x1

    pub value: u16,  // +0x2
    pub index: u16,  // +0x4
    pub length: u16, // +0x6
}

/// Enum representing USB device requests.
#[repr(u8)]
#[derive(Debug, Clone, Copy)]
pub enum UsbDeviceRequestRequest {
    // USB requests
    GetStatus = 0,
    ClearFeature = 1,
    SetFeature = 3,
    SetAddress = 5,
    GetDescriptor = 6,
    SetDescriptor = 7,
    GetConfiguration = 8,
    SetConfiguration = 9,
    GetInterface = 10,
    SetInterface = 11,
    SynchFrame = 12,
    // // HID requests
    // GetReport = 1,
    GetIdle = 2,
    // GetProtocol = 3,
    // SetReport = 9,
    // SetIdle = 10,
    // SetProtocol = 11,
}

impl Default for UsbDeviceRequestRequest {
    fn default() -> Self {
        UsbDeviceRequestRequest::GetStatus
    }
}

impl UsbDeviceRequest {
    pub fn new(
        request_type: u8,
        request: UsbDeviceRequestRequest,
        value: u16,
        index: u16,
        length: u16,
    ) -> Self {
        Self {
            request_type,
            request,
            value,
            index,
            length,
        }
    }
}
