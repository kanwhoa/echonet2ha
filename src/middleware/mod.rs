//! ECHONET Middleware implementation
//! The middleware only deals with events.

use chrono::Datelike;
use std::fmt::Write;

pub mod events;
pub mod api;

#[cfg(test)]
mod tests;

/// Maximum size of an EPC data block
/// Needs two bytes for the type and length
const MAX_EPC_LEN: usize = 0xfd;

// Node types
const NODE_TYPE_GENERAL: u8 = 0x01;
const NODE_TYPE_TRANSMIT_ONLY: u8 = 0x02;

// What message types are supported
const NODE_MESSAGE_FORMAT_SPECIFIED: u8 = 0x01;
const NODE_MESSAGE_FORMAT_ARBITRARY: u8 = 0x02;

// EPC properties
const EPC_OPERATING_STATUS: u8 = 0x80;
const EPC_VERSION_INFORMATION: u8 = 0x82;
const EPC_IDENTIFICATION_NUMBER: u8 = 0x83;
const EPC_FAULT_STATUS: u8 = 0x88;
const EPC_FAULT_CONTENT: u8 = 0x89;
const EPC_MANUFACTURER_CODE: u8 = 0x8a;
const EPC_BUSINESS_FACILITY_CODE: u8 = 0x8b;
const EPC_PRODUCT_CODE: u8 = 0x8c;
const EPC_PRODUCTION_NUMBER: u8 = 0x8d;
const EPC_PRODUCTION_DATE: u8 = 0x8e;
const EPC_STATUS_CHANGE_ANNOUNCEMENT_PROPERTY_MAP: u8 = 0x9d;
const EPC_SET_PROPERTY_MAP: u8 = 0x9e;
const EPC_GET_PROPERTY_MAP: u8 = 0x9f;
const EPC_UNIQUE_IDENTIFIER_DATA: u8 = 0xbf;
const EPC_NUMBER_OF_SELFNODE_INSTANCES: u8 = 0xd3;
const EPC_NUMBER_OF_SELFNODE_CLASSES: u8 = 0xd4;
const EPC_INSTANCE_LIST_NOTIFICATION: u8 = 0xd5;
const EPC_SELFNODE_INSTANCE_LIST_S: u8 = 0xd6;
const EPC_SELFNODE_CLASS_LIST_S: u8 = 0xd7;

// EPC property values
const EPC_OPERATION_STATUS_ON: u8 = 0x30;
const EPC_OPERATION_STATUS_OFF: u8 = 0x31;
const EPC_FAULT_ENCOUNTERED: u8 = 0x41;
const EPC_FAULT_NOT_ENCOUNTERED: u8 = 0x42;

/// Node physical addresses
#[derive(PartialEq, Eq, Debug)]
enum NodeAddress {
    Localhost,
    IPv4(), // sock addr + interface, or ??
    IPV6(),
    Broadcast(), // Does not need an addr, it uses all.
    Serial(String),
}

// Node capabilities
#[derive(PartialEq, Eq, Debug)]
#[repr(u8)]
enum NodeType {
    General = NODE_TYPE_GENERAL,
    TransmitOnly = NODE_TYPE_TRANSMIT_ONLY
}

#[derive(PartialEq, Eq, Debug)]
enum NodePropertyOperation {
    /// The Get or Set operation is NOT supported
    NotSupported,
    /// The Get or Set operation is supported
    Supported,
    /// The Get or Set operation is mandatory. Implies supported.
    Mandatory,
}

// What capabilities does an property support
const NODE_VALUE_OPERATION_GET_AVAILABLE: u8 = 0x01;
const NODE_VALUE_OPERATION_GET_SUPPORTED: u8 = 0x02;
const NODE_VALUE_OPERATION_GET_MANDATORY: u8 = 0x04;
const NODE_VALUE_OPERATION_SET_AVAILABLE: u8 = 0x10;
const NODE_VALUE_OPERATION_SET_SUPPORTED: u8 = 0x20;
const NODE_VALUE_OPERATION_SET_MANDATORY: u8 = 0x40;

// helper structs
/// Holder for the version information and message types
struct NodeEchoNetLiteSupportedVersion {
    major_version: u8,
    minor_version: u8,
    specified_message: bool,
    arbiturary_message: bool,
}

/// Holder for the supported EPC property maps
struct NodePropertyMap {
    /// byte 0xn0 + 0x80 operations
    operations: [u16; 8]
}

impl NodePropertyMap {
    /*
    fn setOperation(&mut self, operation: u8) -> Result<(), api::EpcError> {
        if operation < 0x80 {
            // FIXME: error handling
            return Err(api::EpcError::InvalidValue(0x00));
        }

        self.operations[((operation - 0x80) >> 8) as usize] |= 0x0001 << (operation & 0x0f);
        Ok(())
    }

    fn clearOperation(&mut self, operation: u8) -> Result<(), api::EpcError> {
        if operation < 0x80 {
            // FIXME: error handling
            return Err(api::EpcError::InvalidValue(0x00));
        }

        self.operations[((operation - 0x80) >> 8) as usize] &= !(0x0001 << (operation & 0x0f));
        Ok(())
    }
    */
}


/// Traits to represent the canonical data format. Vec<u8> is the generic form.
trait NodePropertyCanonicalType {}
impl NodePropertyCanonicalType for Vec<u8> {}
impl NodePropertyCanonicalType for bool {}
impl NodePropertyCanonicalType for chrono::NaiveDate {}
impl NodePropertyCanonicalType for String {}
impl NodePropertyCanonicalType for NodeEchoNetLiteSupportedVersion {}
trait NodePropertyGenericType: std::any::Any {
    fn get_epc(&self) -> u8;
    fn as_any(&self) -> &dyn std::any::Any;
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}

// The type aliases the to_canonical and from_canonical functions. This is because multiple traits are not allowed
// on the definition, but it also works to keep consistency. Closing the closure is very difficult and requires
// a bunch of hoops. As such, we'll just use a Reference Count (Rc) to store against the original.
/// Converter function from canonical to internal
type FromCanonicalType<T: NodePropertyCanonicalType> = fn(&NodeProperty<T>, &T) -> Result<Vec<u8>, api::EpcError>;
/// Converter function from internal to canonical
type ToCanonicalType<T: NodePropertyCanonicalType> = fn(&NodeProperty<T>, &[u8]) -> Result<T, api::EpcError>;
/// Validator for internal values, i.e. when recieved from the wire.
type ValidatorType<T: NodePropertyCanonicalType> = fn(&NodeProperty<T>, &[u8]) -> bool;

/// Generic definition of a ECHONET property. This allows for dynamically generating all of the object classes.
struct NodeProperty<T: NodePropertyCanonicalType>
{
    /// Name of the property. Forcing static strings to avoid lifetime issues.
    name: &'static str,
    /// The EPC code it is preresented by
    epc: u8,
    /// If the property is mandatory (i.e. must exist). Specific operations can also have mandatory values
    mandatory: bool,
    /// If the property requires an announce message on change
    announce: bool,
    /// If the property is Get/Set-able. This is used to control whether a message from Home Assistant will generate
    /// a ECHONET Lite message. I.e. if Home Assistant does a get, and that is not supported, then it will not create
    /// a network message. If the Home Assistant does a set, but the value is not settable, then it will result in an
    /// error to Home Assistant not a network message.
    /// See the NODE_VALUE_OPERATION_ constants.
    operations: std::cell::Cell<u8>,
    /// The last known value. Only used for reference, and never an authoritative response.
    /// We will always store in the network format since it is ready to be used immediately.
    last_value: std::cell::RefCell<Vec<u8>>,
    /// When the value was last updated (get or set) in ms from the epoch at UTC.
    /// A last updated value of 0 means that the value has never been updated, and hence the value is
    /// the default. No issue for Set. Get should return error.
    last_updated: std::cell::Cell<i64>,
    /// Convert the canonical type to a ECHONET Lite format. Result is used for storing, so passes ownership
    from_canonical: FromCanonicalType<T>,
    /// Convert the type from an ECHONET Lite format canonical format. Result is a different representation.
    /// Converters should perform their own validation on inputs.
    to_canonical: ToCanonicalType<T>,
    /// Validate an internal value
    validator: ValidatorType<T>
    
}

/// The holder type
impl<T: NodePropertyCanonicalType + 'static> NodePropertyGenericType for NodeProperty<T> {
    fn get_epc(&self) -> u8 {
        self.epc
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// Clone implementation
impl<T: NodePropertyCanonicalType> Clone for NodeProperty<T> {
    fn clone(&self) -> Self {
        Self { 
            name: self.name,
            epc: self.epc.clone(),
            mandatory: self.mandatory.clone(),
            announce: self.announce.clone(),
            operations: self.operations.clone(),
            last_value: std::cell::RefCell::new(Vec::new()),
            last_updated: std::cell::Cell::new(0),
            from_canonical: self.from_canonical.clone(),
            to_canonical: self.to_canonical.clone(),
            validator: self.validator.clone()
        }
    }
}

// General node property implementation
impl<T: NodePropertyCanonicalType> NodeProperty<T> {
    /// Constructor
    const fn new(name: &'static str, epc: u8, announce: bool,
        get_operation: NodePropertyOperation, set_operation: NodePropertyOperation,
        from_canonical: FromCanonicalType<T>, to_canonical: ToCanonicalType<T>,
        validator: ValidatorType<T>) -> NodeProperty<T>
    {       
        Self {
            name: name,
            epc: epc,
            mandatory: match get_operation {
                NodePropertyOperation::Mandatory => true,
                _ => false
            } || match set_operation {
                NodePropertyOperation::Mandatory => true,
                _ => false
            },
            announce: announce,
            operations: std::cell::Cell::new(match get_operation {
                NodePropertyOperation::NotSupported => 0x00,
                NodePropertyOperation::Supported => NODE_VALUE_OPERATION_GET_SUPPORTED,
                NodePropertyOperation::Mandatory => NODE_VALUE_OPERATION_GET_MANDATORY | NODE_VALUE_OPERATION_GET_SUPPORTED,
            } | match set_operation {
                NodePropertyOperation::NotSupported => 0x00,
                NodePropertyOperation::Supported => NODE_VALUE_OPERATION_SET_SUPPORTED,
                NodePropertyOperation::Mandatory => NODE_VALUE_OPERATION_SET_MANDATORY | NODE_VALUE_OPERATION_SET_SUPPORTED,
            }),
            last_value: std::cell::RefCell::new(Vec::new()),
            last_updated: std::cell::Cell::new(0),
            from_canonical: from_canonical,
            to_canonical: to_canonical,
            validator: validator,
        }
    }

    /// Clone an existing property and set the default value from a canonical value
    fn clone_with_canonical_value(&self, value: &T) -> Result<NodeProperty<T>, api::EpcError> {
        let internal = (self.from_canonical)(self, value)?;
        self.clone_with_internal_value(&internal)
    }

    /// Clone an existing property and set the default value from an internal value
    /// Will create a copy of the vec.
    fn clone_with_internal_value(&self, internal: &[u8]) -> Result<NodeProperty<T>, api::EpcError> {
        let cloned = self.clone();
        cloned.set_internal(internal)?;
        Ok(cloned)
    }

    /// Returns true if the get operation is published as available by the end device.
    fn operation_get_available(&self) -> bool { (self.operations.get() & NODE_VALUE_OPERATION_GET_AVAILABLE) == NODE_VALUE_OPERATION_GET_AVAILABLE }
    /// Returns true if the get operation is supported (as per the spec for this class). Note the difference to [get_available].
    fn operation_get_supported(&self) -> bool { (self.operations.get() & NODE_VALUE_OPERATION_GET_SUPPORTED) == NODE_VALUE_OPERATION_GET_SUPPORTED }
    /// Returns true if the get operation is mandatory (as per the spec for this class). Note the difference to [get_available].
    fn operation_get_mandatory(&self) -> bool { (self.operations.get() & NODE_VALUE_OPERATION_GET_MANDATORY) == NODE_VALUE_OPERATION_GET_MANDATORY }

    /// Returns true if the set operation is published as available by the end device.
    fn operation_set_available(&self) -> bool { (self.operations.get() & NODE_VALUE_OPERATION_SET_AVAILABLE) == NODE_VALUE_OPERATION_SET_AVAILABLE }
    /// Returns true if the set operation is supported (as per the spec for this class). Note the difference to [get_available].
    fn operation_set_supported(&self) -> bool { (self.operations.get() & NODE_VALUE_OPERATION_SET_SUPPORTED) == NODE_VALUE_OPERATION_SET_SUPPORTED }
    /// Returns true if the set operation is mandatory (as per the spec for this class). Note the difference to [get_available].
    fn operation_set_mandatory(&self) -> bool { (self.operations.get() & NODE_VALUE_OPERATION_SET_MANDATORY) == NODE_VALUE_OPERATION_SET_MANDATORY }

    /// Sets the get availability
    fn operation_get_enable(&self) { self.operations.set(self.operations.get() | NODE_VALUE_OPERATION_GET_AVAILABLE) }
    /// Sets the set availability
    fn operation_set_enable(&self) { self.operations.set(self.operations.get() | NODE_VALUE_OPERATION_SET_AVAILABLE) }

    /// Determine if a value is appropraite for this EPC
    #[inline(always)]
    fn accept(&self, epc_buf: &Vec<u8>) -> bool {
        epc_buf.len() >= 2 && epc_buf[0] == self.epc
    }

    /// Getter function. Takes the raw EPC buffer and validates/returns it. It will
    /// check the size before actually setting. This set assumes data going to the ECHONET Lite node
    fn get(&self) -> Result<Vec<u8>, api::EpcError> {
        let internal = self.get_internal()?;

        // Package into a wire value
        let mut epc_buf: Vec<u8> = Vec::with_capacity(internal.len() + 2);
        epc_buf[0] = self.epc;
        epc_buf[1] = epc_buf.len() as u8;
        let epc_buf_data = &mut epc_buf[2..][..internal.len()];
        epc_buf_data.copy_from_slice(internal.as_slice());

        Ok(epc_buf)
    }

    /// Setter. Note that permissions are not checked here because they are not equal. I.e. the Home Assistant site should have permissions applied
    /// but the ECHONET side should not as it is only a mirror of the device state. Use Vec as the in/out because it is basically a packed structure
    /// but also has the length accessible. This set assumes data coming from the ECHONET Lite node.
    fn set(&self, epc_buf: &Vec<u8>) -> Result<(), api::EpcError> {
        let epc_buf_len = epc_buf.len();
        // Min of two bytes to get the EPC code and length
        if epc_buf_len < 2 {
            return Err(api::EpcError::InvalidValue(self.epc));
        }

        // Check for this EPC
        if epc_buf[0] != self.epc {
            return Err(api::EpcError::InvalidCode(self.epc, epc_buf[0]));
        }

        // Check for the correct sizing
        let data_size = epc_buf[2] as usize;
        if data_size + 2 != epc_buf_len {
            return Err(api::EpcError::InvalidValue(self.epc));
        }

        if data_size == 0 {
            unimplemented!("Zero length EPC data")
        }
        self.set_internal(&epc_buf[2..])
    }

    /// Get and return an owned canonical version of the internal struct
    fn get_canonical(&self) -> Result<T, api::EpcError> {
        // Check the value exists
        if self.last_updated.get() == 0 {
            return Err(api::EpcError::NoValue(self.epc));
        }

        // Validate the internal just in case someone has set manually
        let cell_value: std::cell::Ref<'_, Vec<u8>> = self.last_value.borrow();
        let internal = cell_value.as_slice();
        if (self.validator)(self, internal) {
            (self.to_canonical)(self, internal)
        } else {
            Err(api::EpcError::ValidationFailed(self.epc))
        }
    }

    /// Set the internal state from a canonical version of the data
    fn set_canonical(&self, canonical: &T) -> Result<(), api::EpcError> {
        // Duplicated internals to avoid a copy
        let internal = (self.from_canonical)(self, canonical)?;
        if internal.len() > MAX_EPC_LEN {
            return Err(api::EpcError::ValueTooLarge(self.epc)); 
        }

        // Validate and save. We can validate the original to avoid closing before valid.
        if (self.validator)(self, internal.as_slice()) {
            let mut cell_value = self.last_value.borrow_mut();
            *cell_value = internal;
            self.last_updated.set(chrono::Utc::now().timestamp_millis());
            Ok(())
        } else {
            Err(api::EpcError::ValidationFailed(self.epc))
        }
    }

    /// Return a borrowed reference to the underlying vec. Cannot return a slice becuase
    /// of lifetime issues.
    fn get_internal(&self) -> Result<std::cell::Ref<'_, Vec<u8>>, api::EpcError> {
        if self.last_updated.get() == 0 {
            return Err(api::EpcError::NoValue(self.epc));
        }
        Ok(self.last_value.borrow())
    }

    /// Set a value. The value is cloned.
    fn set_internal(&self, internal: &[u8]) -> Result<(), api::EpcError> {
        if internal.len() > MAX_EPC_LEN {
            return Err(api::EpcError::ValueTooLarge(self.epc)); 
        }
        // Validate and save. We can validate the original to avoid closing before valid.
        if (self.validator)(self, internal) || internal.len() > MAX_EPC_LEN {
            let mut cell_value = self.last_value.borrow_mut();
            *cell_value = internal.to_vec();
            self.last_updated.set(chrono::Utc::now().timestamp_millis());
            Ok(())
        } else {
            Err(api::EpcError::ValidationFailed(self.epc))
        }
    }
}

// Converters. Some of these allow a validator, which validates the input, i.e. from_xxx -> canonical; to_xxx -> internal.
// These are not meant to replace the validator item in the [NodeProperty] struct, which does full validation of the internal
// value before setting. This is not efficient, but it is tolerant.
/// Convert from a hext strin g(not padded)
/// * `len`: the length of the byte buffer.
#[inline(always)]
fn from_hex(np: &NodeProperty<String>, canonical: &str, len: usize, validate: fn(&str) -> bool) -> Result<Vec<u8>, api::EpcError> {
    // Validate that the input is a hex string
    let mut start: usize = 0;
    for c in canonical.chars() {
        if !((c >= '0' && c <= '9') || (c >= 'a' && c <= 'f') || (c >= 'A' && c <= 'F')) {
            return Err(api::EpcError::InvalidValue(np.epc)); 
        }
        if c == '0' {
            start += 1;
        }
    }

    // Allocate
    let non_padded_canonical = &canonical[start..];
    if non_padded_canonical.len() > (len * 2) || !validate(canonical) {
        return Err(api::EpcError::InvalidValue(np.epc));
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
fn to_hex(np: &NodeProperty<String>, internal: &[u8], len: usize, validate: fn(&[u8]) -> bool) -> Result<String, api::EpcError> {
    if internal.len() > len || !validate(internal) {
        return Err(api::EpcError::InvalidValue(np.epc));
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
fn from_vec(np: &NodeProperty<Vec<u8>>, canonical: &Vec<u8>, validate: fn(&Vec<u8>) -> bool) -> Result<Vec<u8>, api::EpcError> {
    if !validate(canonical) {
        return Err(api::EpcError::InvalidValue(np.epc));
    }
    Ok(canonical.clone())
}
/// To a Vec<u8> cononical type. These are for actual values that should be binary. For hex strings, use to_hex.
#[inline(always)]
fn to_vec(np: &NodeProperty<Vec<u8>>, internal: &Vec<u8>, validate: fn(&[u8]) -> bool) -> Result<Vec<u8>, api::EpcError> {
    if !validate(internal) {
        return Err(api::EpcError::InvalidValue(np.epc));
    }
    Ok(internal.clone())
}

/// From a date without timezone canonical type.
#[inline(always)]
fn from_date(np: &NodeProperty<chrono::NaiveDate>, canonical: &chrono::NaiveDate, validate: fn(&chrono::NaiveDate) -> bool) -> Result<Vec<u8>, api::EpcError> {
    if !validate(canonical) {
        return Err(api::EpcError::InvalidValue(np.epc));
    }
    let mut buf = vec![0x00; 4];
    (&mut buf[0..2]).copy_from_slice(&(canonical.year() as i16).to_be_bytes());
    buf[2] = canonical.month() as u8;
    buf[3] = canonical.day() as u8;
    Ok(buf)
}
/// To a date without timezone cononical type.
#[inline(always)]
fn to_date(np: &NodeProperty<chrono::NaiveDate>, internal: &[u8], validate: fn(&[u8]) -> bool) -> Result<chrono::NaiveDate, api::EpcError> {
    if internal.len() != 4 || !validate(internal) {
        return Err(api::EpcError::InvalidValue(np.epc));
    }
    let year = u16::from_be_bytes(internal[0..2].try_into().unwrap()) as i32;
    let month = internal[2] as u32;
    let day = internal[3] as u32;

    let maybe_canonical = chrono::NaiveDate::from_ymd_opt(year, month, day);
    if let Some(canonical) = maybe_canonical {
        Ok(canonical)
    } else {
        return Err(api::EpcError::InvalidValue(np.epc));
    }    
}

/// From a bool canonical type
#[inline(always)]
fn from_bool(_np: &NodeProperty<bool>, canonical: &bool, true_value: u8, false_value: u8) -> Result<Vec<u8>, api::EpcError> {
    Ok([if *canonical { true_value } else { false_value }].to_vec())
}
/// To a bool canonical type
#[inline(always)]
fn to_bool(np: &NodeProperty<bool>, internal: &[u8], true_value: u8, false_value: u8) -> Result<bool, api::EpcError> {
    if internal.len() == 1 {
        if internal[0] == true_value {
            Ok(true)
        } else if internal[1] == false_value {
            Ok(false)
        } else {
            Err(api::EpcError::InvalidValue(np.epc))
        }
    } else {
        Err(api::EpcError::InvalidValue(np.epc))
    }
}

// Constant node property types. These must be cloned before use.
/// Operating status. True == ON, False == OFF
const NODE_PROPERTY_OPERATING_STATUS: NodeProperty<bool> = NodeProperty::new(
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
const NODE_PROPERTY_VERSION_INFORMATION: NodeProperty<NodeEchoNetLiteSupportedVersion> = NodeProperty::new(
    "Version Information",
    EPC_VERSION_INFORMATION,
    false,
    NodePropertyOperation::Mandatory,
    NodePropertyOperation::NotSupported,
    |_, canonical| {
        let mut internal = Vec::with_capacity(4);
        internal[0] = canonical.major_version;
        internal[1] = canonical.minor_version;
        internal[2] = if canonical.specified_message {NODE_MESSAGE_FORMAT_SPECIFIED} else {0x00} | if canonical.arbiturary_message {NODE_MESSAGE_FORMAT_ARBITRARY} else {0x00};
        internal[3] = 0x00;
        Ok(internal)
    },
    |_, internal| {
        Ok(NodeEchoNetLiteSupportedVersion {
            major_version: internal[0],
            minor_version: internal[1],
            specified_message: (internal[2] & NODE_MESSAGE_FORMAT_SPECIFIED) == NODE_MESSAGE_FORMAT_SPECIFIED,
            arbiturary_message: (internal[2] & NODE_MESSAGE_FORMAT_ARBITRARY) == NODE_MESSAGE_FORMAT_ARBITRARY,
        })
    },
    |_, internal| internal.len() == 4
);
/// This stores two values. The whole value can be considered the identification number, with the
/// first byte identifying the communication medium. In ECHONET Lite, this is fixed at 0xfe. This also means
/// that the rest of the data is the "manufacturer specified format". This is basically 3 bytes identifying
/// the manufacturer and then the remaining 13 bytes specified by the manufacturer.
const NODE_PROPERTY_IDENTIFICATION_NUMBER: NodeProperty<String> = NodeProperty::new(
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
const NODE_PROPERTY_FAULT_STATUS: NodeProperty<bool> = NodeProperty::new(
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
const NODE_PROPERTY_MANUFACTURER_CODE: NodeProperty<String> = NodeProperty::new(
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
const NODE_PROPERTY_BUSINESS_FACILITY_CODE: NodeProperty<String> = NodeProperty::new(
    "Business facility code (Place of business code)",
    EPC_BUSINESS_FACILITY_CODE,
    false,
    NodePropertyOperation::Mandatory,
    NodePropertyOperation::NotSupported,
    |np, canonical: &String| from_hex(np, canonical, 3, |_| true),
    |np, internal| to_hex(np, internal, 3, |_| true),
    |_, internal| internal.len() == 3
);
/// Product code
/// TODO: solve the string/binary form for this. Doesn't make sense to be having binary floating about.
const NODE_PROPERTY_PRODUCT_CODE: NodeProperty<String> = NodeProperty::new(
    "Product code",
    EPC_PRODUCT_CODE,
    false,
    NodePropertyOperation::Mandatory,
    NodePropertyOperation::NotSupported,
    |np, canonical: &String| from_hex(np, canonical, 12, |_| true),
    |np, internal| to_hex(np, internal, 12, |_| true),
    |_, internal| internal.len() == 12
);
/// Production number. Also known as "Serial number".
const NODE_PROPERTY_PRODUCTION_NUMBER: NodeProperty<String> = NodeProperty::new(
    "Production number (Serial number)",
    EPC_PRODUCTION_NUMBER,
    false,
    NodePropertyOperation::Mandatory,
    NodePropertyOperation::NotSupported,
    |np, canonical: &String| from_hex(np, canonical, 12, |_| true),
    |np, internal| to_hex(np, internal, 12, |_| true),
    |_, internal| internal.len() == 12
);
/// Production date. Also known as "Serial number".
const NODE_PROPERTY_PRODUCTION_DATE: NodeProperty<chrono::NaiveDate> = NodeProperty::new(
    "Production date (Date of manufacture)",
    EPC_PRODUCTION_DATE,
    false,
    NodePropertyOperation::Mandatory,
    NodePropertyOperation::NotSupported,
    |np, canonical| from_date(np, canonical, |_| true),
    |np, internal| to_date(np, internal, |_| true),
    |_, internal| internal.len() == 8
);


/// ECHONET Object representation (both device and profile)
struct NodeObject {
    eoj: api::EOJ,
    properties: Vec<Box<dyn NodePropertyGenericType>>
}

impl NodeObject {
    /// Create a device object
    fn device(group_class: &api::GroupClass, instance: u8) -> Self {
        // All objects has a standard set of superclass objects
        // APPENDIX Detailed Requirements for ECHONET Device objects: Device Object Super Class Requirements

        NodeObject {
            eoj: api::EOJ::from_groupclass_instance(group_class, instance),
            properties: vec![
            ]
        }
    }

    /// Create a profile object
    fn profile(r#type: NodeType, manufacturer_code: &[u8; 3], instance: u64) -> Self {
        // All objects has a standard set of superclass objects
        // Communication Middleware Specifications: Profile Object Class Group Specifications

        // Create the unique identification number for this node (structure is as per the spec)
        let instance_bytes = instance.to_be_bytes();
        let identification_number_size = 17;
        let mut identification_number = Vec::with_capacity(identification_number_size);
        identification_number.resize(identification_number_size, 0x00);
        identification_number[0] = 0xfe;
        identification_number[1..4].copy_from_slice(manufacturer_code);
        identification_number[(identification_number_size - 1 - std::mem::size_of::<u64>())..(identification_number_size-1)].copy_from_slice(&instance_bytes);

        // TODO: instead of having a canonical form of vec, should it be a string of hex, or in the HA adapter, do we convert from hex string to hex array?
        // Construct the node object
        NodeObject {
            eoj: api::EOJ::from_groupclass_instance(
                &api::CLASS_PROFILE_NODE_PROFILE,
                r#type as u8
            ),
            properties: vec![
                // Superclass specifications (6.10.1 Overview of Profile Object Super Class Specifications)
                Box::new(NODE_PROPERTY_FAULT_STATUS.clone_with_canonical_value(&false).unwrap()),
                Box::new(NODE_PROPERTY_MANUFACTURER_CODE.clone_with_internal_value(manufacturer_code).unwrap()),

                // Node profile class (6.11.1 Node Profile Class: Detailed Specifications)
                Box::new(NODE_PROPERTY_OPERATING_STATUS.clone_with_canonical_value(&true).unwrap()),
                Box::new(NODE_PROPERTY_VERSION_INFORMATION.clone_with_canonical_value(&NodeEchoNetLiteSupportedVersion {
                    major_version: api::ECHONET_MAJOR_VERSION,
                    minor_version: api::ECHONET_MINOR_VERSION,
                    specified_message: true,
                    arbiturary_message: false
                }).unwrap()),
                Box::new(NODE_PROPERTY_IDENTIFICATION_NUMBER.clone_with_internal_value(identification_number.as_slice()).unwrap()),
            ]
            // TODO: other properties
        }
    }

    /// Find an EPC property
    fn get_node_property_by_code<T: NodePropertyCanonicalType + 'static>(&self, epc: u8) -> Result<&NodeProperty<T>, api::EpcError> {
        for np in self.properties.iter() {
            if np.get_epc() == epc {
                if let Some(np) = np.as_any().downcast_ref::<NodeProperty<T>>() {
                    return Ok(np);
                } else {
                    return Err(api::EpcError::InvalidType(epc));
                }
            }
        }
        Err(api::EpcError::NotSupported(epc))
    }

    /// Find an EPC property using the const as a reference
    fn get_node_property_by_template<T: NodePropertyCanonicalType + 'static>(&self, property: &NodeProperty<T>) -> Result<&NodeProperty<T>, api::EpcError> {
        let epc = property.epc;
        
        for np in self.properties.iter() {
            if np.get_epc() == epc {
                if let Some(np) = np.as_any().downcast_ref::<NodeProperty<T>>() {
                    return Ok(np);
                } else {
                    return Err(api::EpcError::InvalidType(epc));
                }
            }
        }
        Err(api::EpcError::NotSupported(epc))
    }

}


/// Definition of a node. Nodes can have different traits/classes?
/// Instances may not be in sequential order, for example when there are
/// gaps in the addressing/capabilities.
struct Node {
    /// The pyshical address the device was last seen at. This is not the primary key
    /// since device on the network can change addresses by DHCP (though likely not SLAAC)
    physical_address: NodeAddress,
    /// can be determined from the profile object EOJ. Shortcut.
    r#type: NodeType,
    // ECHONET Lite specific handling.
    // 1. Where a device has multiple device objects for the same profile.
    //    For exanple a presence sensor that has both an PIR and mmWave sensor.
    //    These are both "human detection" and so would fall in the same group
    //    and class, but have difference instances of the same class.
    // 2. A device may additionally have multiple devices of different classes.
    //    For exmaple if a device has both a presence sensor (human detection
    //    sensor) and a Infrared blaster.
    // However, there will always be only one profile object.
    profile_object: NodeObject,
    device_objects: Vec<NodeObject>,
}

impl Node {
    /// The unique identifier for the node. Only 24 bits used, however, storing in a larger
    /// entity. This transparrently provides the EPC 0x83. It returns a copy to of the property
    /// data to avoid lifetie issues.
    fn unique_identifier(&self) -> Result<[u8; 16], api::EpcError> {
        let property_result = self.profile_object.get_node_property_by_template(&NODE_PROPERTY_IDENTIFICATION_NUMBER);
        if let Ok(actual) = property_result {
            let data = actual.get_internal()?;
            Ok((&data[1..]).try_into()?)
        } else {    
            Err(api::EpcError::NotAvailable(EPC_IDENTIFICATION_NUMBER))
        }
    }

    /// Determine if a node is available (looking at the profile object status. Needs to be online and without fault. Fault is optional.)
    fn is_available(&self) -> bool {
        // TODO
        true
    }
}

/// Main middleware entry point. Should only be one of these per running instance.
pub struct Middleware {
    /// A handle for the self node.
    self_node: Node,
    /// Other discovered nodes
    nodes: Vec<Node>,
    // The broadcast queue
    broadcast_tx: tokio::sync::mpsc::Sender<events::Event>
}

/// The middleware implementation
impl Middleware {
    pub fn new(instance: u64, broadcast_tx: tokio::sync::mpsc::Sender<events::Event>) -> Self {
        // Get the manufacturer code from HA. Should allow a value to override?
        let manufacturer_code = &api::ECHONET_MANUFACTURER_CODE_UNREGISTERED;

        // Create the standard middleware instance, and default objects.
        let middleware = Self {
            self_node: Node {
                physical_address: NodeAddress::Localhost,
                r#type: NodeType::General,
                profile_object: NodeObject::profile(NodeType::General, manufacturer_code, instance),
                device_objects: vec![
                    NodeObject::device(&api::CLASS_CONTROL_CONTROLLER, 1)
                ]
            },
            nodes: Vec::new(),
            broadcast_tx: broadcast_tx
        };
        log::info!("Middleware initialised (self node: 1 profile object, {} device objects)", middleware.self_node.device_objects.len());

        middleware
    }

    /// Initialise the middleware
    pub async fn startup(&self) -> Result<(), api::MiddlewareError> {
        // Create an announcement event (initial)
        self.broadcast_tx.send(events::Event::Startup).await?;
        
        // Send an announcement message
        self.broadcast_tx.send(events::Event::Announce(
            self.self_node.profile_object.eoj.clone(),
            api::EOJ::from_groupclass_instance(&api::CLASS_PROFILE_NODE_PROFILE, 0x01)
        )).await?;

        // Set the bridge as online and account that status

        Ok(())
    }

    /// Shutdown the middleware
    pub async fn shutdown(&self) -> Result<(), api::MiddlewareError> {
        log::info!("Middleware shutdown started");

        // Set the bridge to offline.

        // Send an annoucement of the middleware shutting down.

        // Shutdown
        self.broadcast_tx.send(events::Event::Shutdown).await?;

        Ok(())
    }
}


/* Needed?

enum EPCType {
    BYTE,
    BYTEARRAY,
    STRING,
    DATE,
    TIME,
    DATETIME,
    SIGNED_SHORT,
    SIGNED_LONG,
    UNSIGNED_SHORT,
    UNSIGNED_LONG,
}
*/