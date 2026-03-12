//! Constant definitions of properties

#[cfg(test)]
mod tests;

use chrono::Datelike;
use std::fmt::Write;

use super::{NodeProperty, NodePropertyOperation, NodeEchonetLiteSupportedVersion, NodePropertyMap};
use super::api::{EpcError, WirePresentable};

// EPC properties
pub(super) const EPC_OPERATING_STATUS: u8 = 0x80;
pub(super) const EPC_VERSION_INFORMATION: u8 = 0x82;
pub(super) const EPC_IDENTIFICATION_NUMBER: u8 = 0x83;
pub(super) const EPC_FAULT_STATUS: u8 = 0x88;
pub(super) const EPC_FAULT_CONTENT: u8 = 0x89;
pub(super) const EPC_MANUFACTURER_CODE: u8 = 0x8a;
pub(super) const EPC_BUSINESS_FACILITY_CODE: u8 = 0x8b;
pub(super) const EPC_PRODUCT_CODE: u8 = 0x8c;
pub(super) const EPC_PRODUCTION_NUMBER: u8 = 0x8d;
pub(super) const EPC_PRODUCTION_DATE: u8 = 0x8e;
pub(super) const EPC_ANNOUNCEMENT_PROPERTY_MAP: u8 = 0x9d;
pub(super) const EPC_SET_PROPERTY_MAP: u8 = 0x9e;
pub(super) const EPC_GET_PROPERTY_MAP: u8 = 0x9f;
pub(super) const EPC_UNIQUE_IDENTIFIER_DATA: u8 = 0xbf;
pub(super) const EPC_NUMBER_OF_SELFNODE_INSTANCES: u8 = 0xd3;
pub(super) const EPC_NUMBER_OF_SELFNODE_CLASSES: u8 = 0xd4;
pub(super) const EPC_INSTANCE_LIST_NOTIFICATION: u8 = 0xd5;
pub(super) const EPC_SELFNODE_INSTANCE_LIST_S: u8 = 0xd6;
pub(super) const EPC_SELFNODE_CLASS_LIST_S: u8 = 0xd7;

// EPC boolean property values
const EPC_OPERATION_STATUS_ON: u8 = 0x30;
const EPC_OPERATION_STATUS_OFF: u8 = 0x31;
const EPC_FAULT_ENCOUNTERED: u8 = 0x41;
const EPC_FAULT_NOT_ENCOUNTERED: u8 = 0x42;

// Property converters
// Converters. Some of these allow a validator, which validates the input, i.e. from_xxx -> canonical; to_xxx -> internal.
// These are not meant to replace the validator item in the [NodeProperty] struct, which does full validation of the internal
// value before setting. This is not efficient, but it is tolerant.

/// Convert from a hex string
/// * `len`: the length of the byte buffer.
#[inline(always)]
fn from_hex(np: &NodeProperty<String>, canonical: &str, len: usize, validate: fn(&str) -> bool) -> Result<Vec<u8>, EpcError> {
    // Validate that the input is a hex string
    let mut start: usize = 0;
    for c in canonical.chars() {
        if !((c >= '0' && c <= '9') || (c >= 'a' && c <= 'f') || (c >= 'A' && c <= 'F')) {
            return Err(EpcError::InvalidValue(np.epc)); 
        }
        if c == '0' {
            start += 1;
        }
    }

    // Allocate
    let non_padded_canonical = &canonical[start..];
    if non_padded_canonical.len() > (len * 2) || !validate(canonical) {
        return Err(EpcError::InvalidValue(np.epc));
    }
    let mut internal = vec![0; len];

    if internal.len() > 0 {
        // Assume not padded, so fill from the right.
        // Offset of 1 if the length is even.
        let offset: usize = if (non_padded_canonical.len() & 0x01) == 0x00 {1} else {0};

        for i in (0..=non_padded_canonical.len()).rev().step_by(2) {
            if 0 == i {
                break; // Need to do this to catch odd size strings
            }

            let pos = (i as isize / 2) - (offset as isize);
            let start = std::cmp::max((i as isize)-2, 0) as usize;
            internal[pos as usize] = u8::from_str_radix(&non_padded_canonical[start..i], 16)?;
        }
    }
    Ok(internal)
}
/// To a hex string cononical type.
/// * `len`: the length of the byte buffer, which can be larger than the internal size.
#[inline(always)]
fn to_hex(np: &NodeProperty<String>, internal: &[u8], len: usize, validate: fn(&[u8]) -> bool) -> Result<String, EpcError> {
    if internal.len() > len || !validate(internal) {
        return Err(EpcError::InvalidValue(np.epc));
    }

    let mut canonical = String::with_capacity(len * 2);

    let pad_len = len - internal.len();
    let pad_str = "00";
    for i in 0..len {
        if i < pad_len {
            canonical.push_str(pad_str);
        } else {
            let byte = &internal[i-pad_len];
            write!(&mut canonical, "{:02x}", byte).expect("Unable to write");
        }
    }
    Ok(canonical)
}

/// From a Vec<u8> canonical type. These are for actual values that should be binary. For hex strings, use from_hex.
#[inline(always)]
fn from_vec(np: &NodeProperty<Vec<u8>>, canonical: &Vec<u8>, validate: fn(&Vec<u8>) -> bool) -> Result<Vec<u8>, EpcError> {
    if !validate(canonical) {
        return Err(EpcError::InvalidValue(np.epc));
    }
    Ok(canonical.clone())
}
/// To a Vec<u8> cononical type. These are for actual values that should be binary. For hex strings, use to_hex.
#[inline(always)]
fn to_vec(np: &NodeProperty<Vec<u8>>, internal: &Vec<u8>, validate: fn(&[u8]) -> bool) -> Result<Vec<u8>, EpcError> {
    if !validate(internal) {
        return Err(EpcError::InvalidValue(np.epc));
    }
    Ok(internal.clone())
}

/// From a date without timezone canonical type.
#[inline(always)]
fn from_date(np: &NodeProperty<chrono::NaiveDate>, canonical: &chrono::NaiveDate, validate: fn(&chrono::NaiveDate) -> bool) -> Result<Vec<u8>, EpcError> {
    if !validate(canonical) {
        return Err(EpcError::InvalidValue(np.epc));
    }
    let mut buf = vec![0x00; 4];
    (&mut buf[0..2]).copy_from_slice(&(canonical.year() as i16).to_be_bytes());
    buf[2] = canonical.month() as u8;
    buf[3] = canonical.day() as u8;
    Ok(buf)
}
/// To a date without timezone cononical type.
#[inline(always)]
fn to_date(np: &NodeProperty<chrono::NaiveDate>, internal: &[u8], validate: fn(&[u8]) -> bool) -> Result<chrono::NaiveDate, EpcError> {
    if internal.len() != 4 || !validate(internal) {
        return Err(EpcError::InvalidValue(np.epc));
    }
    let year = u16::from_be_bytes(internal[0..2].try_into().unwrap()) as i32;
    let month = internal[2] as u32;
    let day = internal[3] as u32;

    let maybe_canonical = chrono::NaiveDate::from_ymd_opt(year, month, day);
    if let Some(canonical) = maybe_canonical {
        Ok(canonical)
    } else {
        return Err(EpcError::InvalidValue(np.epc));
    }    
}

/// From a bool canonical type
#[inline(always)]
fn from_bool(_np: &NodeProperty<bool>, canonical: &bool, true_value: u8, false_value: u8) -> Result<Vec<u8>, EpcError> {
    Ok([if *canonical { true_value } else { false_value }].to_vec())
}
/// To a bool canonical type
#[inline(always)]
fn to_bool(np: &NodeProperty<bool>, internal: &[u8], true_value: u8, false_value: u8) -> Result<bool, EpcError> {
    if internal.len() == 1 {
        if internal[0] == true_value {
            Ok(true)
        } else if internal[1] == false_value {
            Ok(false)
        } else {
            Err(EpcError::InvalidValue(np.epc))
        }
    } else {
        Err(EpcError::InvalidValue(np.epc))
    }
}

// Constant node property types. These must be cloned before use.
/// Operating status. True == ON, False == OFF
pub(super) const NODE_PROPERTY_OPERATING_STATUS: NodeProperty<bool> = NodeProperty::new(
    "Operating Status",
    EPC_OPERATING_STATUS,
    true,
    NodePropertyOperation::Mandatory,
    NodePropertyOperation::Supported,
    |np, canonical| from_bool(np, canonical, EPC_OPERATION_STATUS_ON, EPC_OPERATION_STATUS_OFF),    
    |np, internal| to_bool(np, internal, EPC_OPERATION_STATUS_ON, EPC_OPERATION_STATUS_OFF),
    |_, internal| internal == [EPC_OPERATION_STATUS_ON] || internal == [EPC_OPERATION_STATUS_OFF]
);

/// This type is special. It contains the version supported AND the message type supported. However,
/// in ECHONET Lite, only the "specified message format" is supported, meaning that the if the device
/// advertisies "arbitrary message format", chances are we won't be able to interpret any messages from
/// the device. The actual message format is stored in EHD1/EHD2 headers. For here, we will just store
/// the value.
pub(super) const NODE_PROPERTY_VERSION_INFORMATION: NodeProperty<NodeEchonetLiteSupportedVersion> = NodeProperty::new(
    "Version Information",
    EPC_VERSION_INFORMATION,
    false,
    NodePropertyOperation::Mandatory,
    NodePropertyOperation::NotSupported,
    |_, canonical| Ok(canonical.to_wire()?), 
    |_, internal| Ok(NodeEchonetLiteSupportedVersion::from_wire(internal)?),    
    |_, internal| internal.len() == 4
);

/// This stores two values. The whole value can be considered the identification number, with the
/// first byte identifying the communication medium. In ECHONET Lite, this is fixed at 0xfe. This also means
/// that the rest of the data is the "manufacturer specified format". This is basically 3 bytes identifying
/// the manufacturer and then the remaining 13 bytes specified by the manufacturer.
pub(super) const NODE_PROPERTY_IDENTIFICATION_NUMBER: NodeProperty<String> = NodeProperty::new(
    "Version Information",
    EPC_IDENTIFICATION_NUMBER,
    false,
    NodePropertyOperation::Mandatory,
    NodePropertyOperation::NotSupported,
    |np, canonical: &String| from_hex(np, canonical, 17, |_| true),
    |np, internal| to_hex(np, internal, 17, |_| true),
    |_, internal| internal.len() == 17 && internal[0] == 0xfe
);

/// The fault status. true == fault. False ==  no fault.
pub(super) const NODE_PROPERTY_FAULT_STATUS: NodeProperty<bool> = NodeProperty::new(
    "Fault Status",
    EPC_FAULT_STATUS,
    true,
    NodePropertyOperation::Supported,
    NodePropertyOperation::NotSupported,
    |np, canonical| from_bool(np, canonical, EPC_FAULT_ENCOUNTERED, EPC_FAULT_NOT_ENCOUNTERED),    
    |np, internal| to_bool(np, internal, EPC_FAULT_ENCOUNTERED, EPC_FAULT_NOT_ENCOUNTERED),
    |_, internal| internal == [EPC_FAULT_ENCOUNTERED] || internal == [EPC_FAULT_NOT_ENCOUNTERED]
);

/// Manufacturer code. See also [NODE_PROPERTY_IDENTIFICATION_NUMBER]
pub(super) const NODE_PROPERTY_MANUFACTURER_CODE: NodeProperty<String> = NodeProperty::new(
    "Manufacturer code",
    EPC_MANUFACTURER_CODE,
    false,
    NodePropertyOperation::Mandatory,
    NodePropertyOperation::NotSupported,
    |np, canonical: &String| from_hex(np, canonical, 3, |_| true),
    |np, internal| to_hex(np, internal, 3, |_| true),
    |_, internal| internal.len() == 3
);

/// Business facility code. Also known as "Place of business code"
pub(super) const NODE_PROPERTY_BUSINESS_FACILITY_CODE: NodeProperty<String> = NodeProperty::new(
    "Business facility code (Place of business code)",
    EPC_BUSINESS_FACILITY_CODE,
    false,
    NodePropertyOperation::Supported,
    NodePropertyOperation::NotSupported,
    |np, canonical: &String| from_hex(np, canonical, 3, |_| true),
    |np, internal| to_hex(np, internal, 3, |_| true),
    |_, internal| internal.len() == 3
);

/// Product code
pub(super) const NODE_PROPERTY_PRODUCT_CODE: NodeProperty<String> = NodeProperty::new(
    "Product code",
    EPC_PRODUCT_CODE,
    false,
    NodePropertyOperation::Supported,
    NodePropertyOperation::NotSupported,
    |np, canonical: &String| from_hex(np, canonical, 12, |_| true),
    |np, internal| to_hex(np, internal, 12, |_| true),
    |_, internal| internal.len() == 12
);

/// Production number. Also known as "Serial number".
pub(super) const NODE_PROPERTY_PRODUCTION_NUMBER: NodeProperty<String> = NodeProperty::new(
    "Production number (Serial number)",
    EPC_PRODUCTION_NUMBER,
    false,
    NodePropertyOperation::Supported,
    NodePropertyOperation::NotSupported,
    |np, canonical: &String| from_hex(np, canonical, 12, |_| true),
    |np, internal| to_hex(np, internal, 12, |_| true),
    |_, internal| internal.len() == 12
);

/// Production date. Also known as "Date of Manufacture".
pub(super) const NODE_PROPERTY_PRODUCTION_DATE: NodeProperty<chrono::NaiveDate> = NodeProperty::new(
    "Production date (Date of manufacture)",
    EPC_PRODUCTION_DATE,
    false,
    NodePropertyOperation::Supported,
    NodePropertyOperation::NotSupported,
    |np, canonical| from_date(np, canonical, |_| true),
    |np, internal| to_date(np, internal, |_| true),
    |_, internal| internal.len() == 4
);

/// Status change announcement property map. Which properties cause an announcement message if changed.
pub(super) const NODE_PROPERTY_ANNOUNCEMENT_PROPERTY_MAP: NodeProperty<NodePropertyMap> = NodeProperty::new(
    "Status change announcement property map",
    EPC_ANNOUNCEMENT_PROPERTY_MAP,
    false,
    NodePropertyOperation::Mandatory,
    NodePropertyOperation::NotSupported,
    |_, canonical| Ok(canonical.to_wire()?), 
    |_, internal| Ok(NodePropertyMap::from_wire(internal)?),    
    |_, internal| internal.len() > 1 && ((internal[0] < 16 && ((internal[0] + 1) as usize == internal.len())) || internal.len() == 17)
);

/// Set property map. Which properties cause an announcement message if changed.
pub(super) const NODE_PROPERTY_SET_PROPERTY_MAP: NodeProperty<NodePropertyMap> = NodeProperty::new(
    "Status change announcement property map",
    EPC_SET_PROPERTY_MAP,
    false,
    NodePropertyOperation::Mandatory,
    NodePropertyOperation::NotSupported,
    |_, canonical| Ok(canonical.to_wire()?), 
    |_, internal| Ok(NodePropertyMap::from_wire(internal)?),    
    |_, internal| internal.len() > 1 && ((internal[0] < 16 && ((internal[0] + 1) as usize == internal.len())) || internal.len() == 17)
);

/// Get property map. Which properties cause an announcement message if changed.
pub(super) const NODE_PROPERTY_GET_PROPERTY_MAP: NodeProperty<NodePropertyMap> = NodeProperty::new(
    "Status change announcement property map",
    EPC_GET_PROPERTY_MAP,
    false,
    NodePropertyOperation::Mandatory,
    NodePropertyOperation::NotSupported,
    |_, canonical| Ok(canonical.to_wire()?), 
    |_, internal| Ok(NodePropertyMap::from_wire(internal)?),    
    |_, internal| internal.len() > 1 && ((internal[0] < 16 && ((internal[0] + 1) as usize == internal.len())) || internal.len() == 17)
);

