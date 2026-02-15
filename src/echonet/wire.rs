//! ECHONET Lite wire represenations
#![allow(dead_code)]
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
    opc: u8
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
    edc: [u8]
}

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
        let layout = std::alloc::Layout::from_size_align(std::mem::size_of::<EchonetFrame>() + property_size, 1).unwrap();

        // Get the transaction ID.
        let tid = maybe_tid.unwrap_or({
            let mut rng = rand::rng();
            rng.random::<u16>()
        });

        return unsafe {
            // Allocate
            let frame_ptr = std::alloc::alloc_zeroed(layout) as *mut EchonetFrame;
            if frame_ptr.is_null() {
                std::alloc::handle_alloc_error(layout);
            }

            // Set the standard fields
            std::ptr::write_unaligned(&raw mut (*frame_ptr).ehd1 as *mut u16, EHD1_ECHONET_LITE | EHD2_FIXED);
            std::ptr::write_unaligned(&raw mut (*frame_ptr).tid, tid);

            // Create a fat pointer. Need to treat as u8 because of the arguments to slice_from_raw_parts_mut.
            let slice_ptr= std::ptr::slice_from_raw_parts_mut(frame_ptr as *mut u8, layout.size());
            // Wrap the raw pointer in a Box for safe ownership and automatic dropping
            Box::from_raw(slice_ptr as *mut EchonetFrame)
        };
    }

    /// Create a new initial broadcast frame
    /// Middleware spec 4.3.1
    pub fn create_all_nodes_query() -> Box<Self> {
        let properties_size: usize = 3; // TBC
        let mut frame_box = Self::create_frame(None, properties_size);
        let frame_ptr = &raw mut *frame_box;

        let object_spec = [EOJ_CLASS_GROUP_PROFILE, EOJ_CLASS_GROUP_PROFILE_NODE_PROFILE, EOJ_CLASS_GROUP_PROFILE_NODE_PROFILE_GENERAL_NODE];
        unsafe {
            // FIXME: Figure 14.4
            std::ptr::write_unaligned(&raw mut (*frame_ptr).edata.seoj, object_spec);
            std::ptr::write_unaligned(&raw mut (*frame_ptr).edata.deoj, object_spec);
            // FIXME
            /*
            frame.edata.esv = 0x73;
            frame.edata.opc = 0x01;
            frame.edata.properties[0].epc = 0xD5;
            frame.edata.properties[0].pdc = 0x01?;
            frame.edata.properties[0].edc = 0x01?;*/            
        }

        return frame_box;
    } 

}

/// Display for packets
impl std::fmt::Display for EchonetFrame {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "TODO: display packet")
    }
}

impl std::fmt::Debug for EchonetFrame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str = "Bla".to_string();
        f.debug_struct("EchonetFrame")
            .field("field1", &str)
            .finish()
    }
}
