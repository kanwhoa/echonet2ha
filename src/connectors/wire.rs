//! ECHONET Lite wire represenations
#![allow(dead_code)]
use macros::Size;
use std::fmt::Write;
use rand::prelude::*;

const EHD1_ECHONET: u16 = 0x0800;
const EHD1_ECHONET_LITE: u16 = 0x1000;
const EHD2_FIXED: u16 = 0x0080;
const ESV_COMMON: u8 = 0x40;
const ESV_COMMON_MASK: u8 = 0xC0;

/// ESV request prefix
const ESV_REQUEST: u8 = 0x20;
/// Property value write request (no response required)
/// Middleware spec table 3.9
const ESV_REQUEST_SETI: u8 = ESV_COMMON | ESV_REQUEST | 0x00;
/// Property value write request (response required)
/// Middleware spec table 3.9
const ESV_REQUEST_SETC: u8 = ESV_COMMON | ESV_REQUEST | 0x01;
/// Property value read request
/// Middleware spec table 3.9
const ESV_REQUEST_GET: u8 = ESV_COMMON | ESV_REQUEST | 0x02;
/// Property value notification request
/// Middleware spec table 3.9
const ESV_REQUEST_INF_REQ: u8 = ESV_COMMON | ESV_REQUEST | 0x03;
/// Property value write & read request
/// Middleware spec table 3.9
const ESV_REQUEST_SETGET: u8 = ESV_COMMON | ESV_REQUEST | 0x0e;

/// ESV response prefix
const ESV_RESPONSE: u8 = 0x30;
/// Property value Property value write response
/// Middleware spec table 3.10
const ESV_RESPONSE_SET_RES: u8 = ESV_COMMON | ESV_RESPONSE | 0x01;
/// Property value read response
/// Middleware spec table 3.10
const ESV_RESPONSE_GET_RES: u8 = ESV_COMMON | ESV_RESPONSE | 0x02;
/// Property value notification
/// Middleware spec table 3.10
const ESV_RESPONSE_INF: u8 = ESV_COMMON | ESV_RESPONSE | 0x03;
/// Property value notification (response required)
/// Middleware spec table 3.10
const ESV_RESPONSE_INFC: u8 = ESV_COMMON | ESV_RESPONSE | 0x04;
/// Property value notification response
/// Middleware spec table 3.10
const ESV_RESPONSE_INFC_RES: u8 = ESV_COMMON | ESV_RESPONSE | 0x0a;
/// Property value write & read response
/// Middleware spec table 3.10
const ESV_RESPONSE_SETGET_RES: u8 = ESV_COMMON | ESV_RESPONSE | 0x0e;

/// ESV error prefix
const ESV_ERROR: u8 = 0x10;
/// Property value write request "response not possible"
/// Middleware spec table 3.11
const ESV_ERROR_SETI_SNA: u8 = ESV_COMMON | ESV_ERROR | 0x00;
/// Property value write request "response not possible"
/// Middleware spec table 3.11
const ESV_ERROR_SETC_SNA: u8 = ESV_COMMON | ESV_ERROR | 0x01;
/// Property value read "response not possible" 
/// Middleware spec table 3.11
const ESV_ERROR_GET_SNA: u8 = ESV_COMMON | ESV_ERROR | 0x02;
/// Property value notification "response not possible" 
/// Middleware spec table 3.11
const ESV_ERROR_INF_SNA: u8 = ESV_COMMON | ESV_ERROR | 0x03;
/// Property value write & read "response not possible" 
/// Middleware spec table 3.11
const ESV_ERROR_SETGET_SNA: u8 = ESV_COMMON | ESV_ERROR | 0x0e;

// Object (EOJ) Group codes.
// Middleware spec table 3.1
const EOJ_CLASS_GROUP_SENSOR: u8 = 0x00;
const EOJ_CLASS_GROUP_AIRCON: u8 = 0x01;
const EOJ_CLASS_GROUP_FACILITY: u8 = 0x02;
const EOJ_CLASS_GROUP_HOUSEWORK: u8 = 0x03;
const EOJ_CLASS_GROUP_HEALTH: u8 = 0x04;
const EOJ_CLASS_GROUP_CONTROL: u8 = 0x05;
const EOJ_CLASS_GROUP_AV: u8 = 0x06;
const EOJ_CLASS_GROUP_PROFILE: u8 = 0x0e;
const EOJ_CLASS_GROUP_USER: u8 = 0x0f;

// TODO: class codes
// Profile class
const EOJ_CLASS_GROUP_PROFILE_NODE_PROFILE: u8 = 0xf0;
const EOJ_CLASS_GROUP_PROFILE_NODE_PROFILE_GENERAL_NODE: u8 = 0x01;
const EOJ_CLASS_GROUP_PROFILE_NODE_PROFILE_TX_ONLY_NODE: u8 = 0x02;

//https://echonet.jp/wp/wp-content/uploads/pdf/General/Standard/ECHONET_lite_V1_14_en/ECHONET-Lite_Ver.1.14(02)_E.pdf
// https://echonet.jp/spec_v114_lite_en/
// https://echonet.jp/wp/wp-content/uploads/pdf/General/Standard/ECHONET_lite_V1_14_en/ECHONET-Lite_Ver.1.14(03)_E.pdf

/// Main ECHONET struct
/// Middleware spec 3.2.1
#[repr(C, packed)]
#[derive(Size)]
pub struct EchonetFrame {
    /// ECHONET header 1
    ehd1: u8,
    /// ECHONET header 2
    ehd2: u8,
    /// ECHONET transaction identifier
    tid: u16,
    // Embedded ECHONET EDATA
    edata: EchonetEdata,
}
// FIXME: from macro
const ECHONETFRAME_SIZE: usize = 12;

/// ECHONET EDATA struct
/// Middleware spec 3.2.3
#[repr(C, packed)]
pub struct EchonetEdata {
    /// Source ECHONET Lite object specification. [Class group code, class code, instance code]
    /// Class group codes: Middleware spec table 3.1
    /// Class code: Middleware spec table 3.2 - table 3.8
    /// instances code: instance of class group and class code (if more than one exists). 0x00 means all instanes.
    seoj: [u8; 3],
    /// Destination ECHONET Lite object specification. [Class group code, class code, instance code]
    deoj: [u8; 3],
    /// ECHONET Lite Service
    /// Middleware spec 3.2.5
    esv: u8,
    /// Number of processing properties
    opc: u8,
    /// Each of the properties. Must store this as u8 and then cast when extracting to avoid lots of compiler issues.
    epcs: [u8]
}

/// ECHONET property struct
/// Middleware spec 3.2.3
#[repr(C, packed)]
pub struct EchonetProperty {
    /// ECHONET property
    epc: u8,
    /// Property Data Counter
    pdc: u8,
    /// Property data value (length is defined in PDC). This is a dynamically-sized type.
    edt: [u8]
}

/// Macro to help with unsafe read access. This is not efficient for large scale writes.
/// TODO: use a "rust macro get type of struct" to get the type and add the ordering.
macro_rules! get_value {
    ($self:ident, $field:ident) => {
        unsafe { (&raw const $self.$field).read_unaligned() }
    };
}

/// Need to create a dummy constant which can then be used to determine the size of the DST.
/// This is so incredibly dumb, but Rust prevents the size_of use with a DST rather than
/// assuming the size_of dynamic part to be 0 length.
/*const SIZED_INSTANCE = Box::new(EchonetFrame {
    ehd1: 0x00,
    ehd2: 0x00,
    tid: 0x0000,
    edata: EchonetEdata {
        seoj: [0x00, 0x00, 0x00],
        deoj: [0x00, 0x00, 0x00],
        esv: 0x00,
        opc: 0x00,
        epcs: [0; 8]
    }
});*/

/// ECHONET frame methods
impl EchonetFrame {
    /// Determine if this frame is an ECHONET format.
    /// Middleware spec 3.2.1
    pub const fn is_echonet(&self) -> bool {
        let ehd_ptr = &raw const self.ehd1 as *const u16;
        let ehd_value = unsafe { ehd_ptr.read_unaligned() };

        (ehd_value & EHD1_ECHONET) == EHD1_ECHONET && (ehd_value & 0x00ff) == EHD2_FIXED
    }

    /// Determine if this frame is ECHONET Lite format.
    /// Middleware spec 3.2.1
    pub const fn is_echonet_lite(&self) -> bool {
        let ehd_ptr = &raw const self.ehd1 as *const u16;
        let ehd_value = unsafe { ehd_ptr.read_unaligned() };

        (ehd_value & EHD1_ECHONET_LITE) == EHD1_ECHONET_LITE && (ehd_value & 0x00ff) == EHD2_FIXED
    }

    /// Create a basic EchoNet Lite frame
    #[inline(always)]
    fn create_frame(maybe_tid: Option<u16>, property_size: usize) -> Box<Self> {
        // Calcaulte the layout size manually to avoid padding by the layout.
        // FIXME: use a macro to get the size of the struct?
        let layout = std::alloc::Layout::from_size_align(ECHONETFRAME_SIZE + property_size, 1).unwrap();
        println!("Layout Size info: {}", layout.size());

        // Get the transaction ID.
        let tid = maybe_tid.unwrap_or({
            let mut rng = rand::rng();
            rng.random::<u16>()
        });

        return unsafe {
            // Allocate. The frame pointer is a standard pointer and does not contain size informaiton.
            let mem_ptr = std::alloc::alloc_zeroed(layout);
            if mem_ptr.is_null() {
                std::alloc::handle_alloc_error(layout);
            }

            // Create a fat pointer.
            let slice_ptr= std::ptr::slice_from_raw_parts_mut(mem_ptr, layout.size());
            let frame_ptr = slice_ptr as *mut EchonetFrame;

            // Set the standard fields
            std::ptr::write_unaligned(&raw mut (*frame_ptr).ehd1 as *mut u16, (EHD1_ECHONET_LITE | EHD2_FIXED).to_be());
            std::ptr::write_unaligned(&raw mut (*frame_ptr).tid, tid.to_be());

            // Wrap the raw pointer in a Box for safe ownership and automatic dropping
            Box::from_raw(frame_ptr)
        };
    }

    /// Create a new initial broadcast frame
    /// Middleware spec 4.3.1
    pub fn create_all_nodes_query() -> Box<Self> {
        let properties_size: usize = 3; // TBC
        let mut frame_box = Self::create_frame(None, properties_size);
        let frame_ptr = &raw mut *frame_box;
        println!("Pointer Size info: {}", std::mem::size_of_val(&*frame_box));

        let object_spec = [EOJ_CLASS_GROUP_PROFILE, EOJ_CLASS_GROUP_PROFILE_NODE_PROFILE, EOJ_CLASS_GROUP_PROFILE_NODE_PROFILE_GENERAL_NODE];
        unsafe {
            // FIXME: Figure 14.4
            std::ptr::write_unaligned(&raw mut (*frame_ptr).edata.seoj, object_spec);
            std::ptr::write_unaligned(&raw mut (*frame_ptr).edata.deoj, object_spec);
            std::ptr::write_unaligned(&raw mut (*frame_ptr).edata.esv, ESV_RESPONSE_INF);
            // FIXME
            /*
            frame.edata.esv = 0x73;
            frame.edata.opc = 0x01;
            frame.edata.properties[0].epc = 0xD5;
            frame.edata.properties[0].pdc = 0x??; // depends on length in edt. See para 6.11.1. Likely this is zero since we only have the node profile and not device profile.
            // Do we want to create a dummy controller class through? Might be an idea, since we only need to send 
            frame.edata.properties[0].edt = 0x01?; // Instance list information code */
            // frame_ptr.add_property(...) <- checks the size, increments the properties and appends.          
        }

        return frame_box;
    }

    /// Get the actual packet length. Cannot use the allocated size as it contains padding which will add extra to the length
    pub fn len(&self) -> usize {
        // Use the caulcated size of header, and access the number of processing entities. Need to iterate through the block
        // which has to be done manually as each is varying size.
        let actual: usize = ECHONETFRAME_SIZE;

        // Get the number of properties and then calcualte the size of each
        let opc_pointer = &raw const self.edata.opc;
        let opc = unsafe { opc_pointer.read_unaligned() };

        for i in 0..opc {
            unimplemented!();
        }

        return actual;
    }
}

/// Utility function to display raw data.
fn display_raw(f: &mut std::fmt::Formatter<'_>, data: &[u8], data_len: usize) -> std::fmt::Result {
        let mut buffer: String = String::with_capacity(100);
        println!("Scanning over {} bytes", data_len);
        f.write_str("\nRaw packet:\n")?;

        // Iterate over data
        for i in 0..data_len {
            if i % 16 == 0 {
                buffer.write_fmt(format_args!(" {:08x}:", i))?;
            }

            buffer.write_fmt(format_args!(" {:02x}", data[i]))?;
            if i % 16 == 7 {
                buffer.write_str("  ")?;
            }

            if i % 16 == 15 {
                buffer.push('\n');
                f.write_str(&buffer)?;
                buffer.clear();
            }
        }
        
        if !buffer.is_empty() {
            buffer.push('\n');
        }
        f.write_str(&buffer)
}

/// Display for packets
impl std::fmt::Display for EchonetFrame {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "EchonetFrame(tid={:x}", u16::from_be(get_value!(self, tid)))
    }
}

/// Display for debug
impl std::fmt::Debug for EchonetFrame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EchonetFrame")
            .field("ehd1", &format_args!("0x{:x}", &get_value!(self, ehd1)))
            .field("ehd2", &format_args!("0x{:x}", &get_value!(self, ehd2)))
            .field("tid", &format_args!("0x{:x}", &u16::from_be(get_value!(self, tid))))
            .field("edata.seoj", &format_args!("0x{:x}", &get_value!(self, ehd1)))
            .finish()?;

        let data_size = std::cmp::min(std::mem::size_of_val(self), self.len());
        let data = self as *const EchonetFrame as *const [u8];
        display_raw(f, unsafe { &*data }, data_size)
    }
}

