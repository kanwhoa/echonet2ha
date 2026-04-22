//! ECHONET related constants
//! Values that are used across wire and middleware implementations.
use std::any::Any;
use std::fmt::{Debug, Display, Write};
use std::mem::MaybeUninit;
use std::ops::{Deref, DerefMut};
use std::str::{FromStr};
use derive_more::{Display, From};

use crate::middleware::epc_types::EPC_FAULT_CONTENT;

#[cfg(test)]
mod tests;

/// ECHONET major version
pub const ECHONET_MAJOR_VERSION: u8 = 1;
/// ECHONET minor version
pub const ECHONET_MINOR_VERSION: u8 = 14;

/// ECHONET manufacturer codes (should be assigned by the ECHONET consortium). We just take one that is likely not to clash.
/// [https://echonet.jp/wp/wp-content/uploads/pdf/General/Echonet/ManufacturerCode/list_code.pdf](Defined codes).
pub const ECHONET_MANUFACTURER_CODE_UNREGISTERED: [u8; 3] = [0xff, 0xff, 0xff];

// Object (EOJ) Group codes.
// Middleware spec table 3.1
pub const EOJ_CLASS_GROUP_SENSOR: u8 = 0x00;
pub const EOJ_CLASS_GROUP_AIRCON: u8 = 0x01;
pub const EOJ_CLASS_GROUP_FACILITY: u8 = 0x02;
pub const EOJ_CLASS_GROUP_HOUSEWORK: u8 = 0x03;
pub const EOJ_CLASS_GROUP_HEALTH: u8 = 0x04;
pub const EOJ_CLASS_GROUP_CONTROL: u8 = 0x05;
pub const EOJ_CLASS_GROUP_AV: u8 = 0x06;
pub const EOJ_CLASS_GROUP_PROFILE: u8 = 0x0e;
pub const EOJ_CLASS_GROUP_USER: u8 = 0x0f;

const UNKNOWN: &str = "Unknown";

///////////////////////////////////////////////////////////////////////////////
// Errors
///////////////////////////////////////////////////////////////////////////////

/// EPC errors
#[derive(Debug)]
pub enum EpcError {
    /// The EPC code is not valid for this property
    InvalidCode(u8, u8),
    /// The value for the EPC code is not correct (type and/or size)
    InvalidValue(u8, String),
    /// Type converstion failed when downcasting to impl.
    InvalidType(u8),
    /// The EPC is not implemented by the node. We follow the spec, so all properties should be listed.
    NotAvailable(u8),
    /// This EPC is not supported on this object (as per the specification)
    NotSupported(u8),
    /// The operation is not allowed by an access rule
    OperationNotAllowed(u8),
    /// The operation is not implemented by the node for this EPC
    OperationNotImplemented(u8),
    /// The value has not been set yet
    NoValue(u8),
    /// The value is too large (overflow)
    ValueTooLarge(u8, String),
    /// Validation of the value failed.
    ValidationFailed(u8),
    /// Error when converting the value between types
    TypeConverstionError(u8, String),
    /// Error when obtaining the value
    ValueError(u8, String)
}

impl std::fmt::Display for EpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            EpcError::InvalidCode(epc, actual_epc) => write!(f, "EPC({:02x}): Actual code: {:02x}", epc, actual_epc),
            EpcError::InvalidValue(epc, msg) => write!(f, "EPC({:02x}): Invalid value - {}", epc, msg),
            EpcError::InvalidType(epc) => write!(f, "EPC({:02x}): Mismatched canonical type", epc),
            EpcError::NotAvailable(epc) => write!(f, "EPC({:02x}): EPC is not available on this device", epc),
            EpcError::NotSupported(epc) => write!(f, "EPC({:02x}): EPC is not supported on this object class", epc),
            EpcError::OperationNotAllowed(epc) => write!(f, "EPC({:02x}): Operation not allowed by access rule", epc),
            EpcError::OperationNotImplemented(epc) => write!(f, "EPC({:02x}): Operation not implemented by node", epc),
            EpcError::NoValue(epc) => write!(f, "EPC({:02x}): Value is not set", epc),
            EpcError::ValueTooLarge(epc, msg) => write!(f, "EPC({:02x}): Value is larger than the container maximum: {}", epc, msg),
            EpcError::ValidationFailed(epc) => write!(f, "EPC({:02x}): Validation for internal representation failed", epc),
            EpcError::TypeConverstionError(epc, msg) => write!(f, "EPC({:02x}): Failed to convert internal type: {}", epc, msg),
            EpcError::ValueError(epc, msg) => write!(f, "EPC({:02x}): Failed to obtain value: {}", epc, msg),
        }
    }
}

impl std::error::Error for EpcError {}

#[derive(Debug)]
pub enum MiddlewareError {
    /// A communications error on a channel or socket
    QueueFailure(String),
    /// An invalid value was propvided
    InvalidValue(String),
}

impl std::fmt::Display for MiddlewareError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            MiddlewareError::QueueFailure(msg) => write!(f, "Queue message failure: {}", msg),
            MiddlewareError::InvalidValue(msg) => write!(f, "An invalid value was provided: {}", msg),
        }
    }
}

impl std::error::Error for MiddlewareError {}

impl<T> From<tokio::sync::mpsc::error::SendError<T>> for MiddlewareError {
    fn from(value: tokio::sync::mpsc::error::SendError<T>) -> Self {
        MiddlewareError::QueueFailure(format!("Failed to send '{}' message to queue as channel was closed", value.to_string()))
    }
}


/// HexString errors
#[derive(Debug)]
pub enum HexStringError {
    /// The source contained invalid characters
    InvalidCharacter,
    /// The buffer length is too small
    BufferTooSmall,
}

impl std::fmt::Display for HexStringError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            HexStringError::InvalidCharacter => write!(f, "Invalid characters in hex value"),
            HexStringError::BufferTooSmall => write!(f, "The value is larger than the buffer allows"),
        }
    }
}

impl std::error::Error for HexStringError {}

impl From<std::num::ParseIntError> for HexStringError {
    fn from(_: std::num::ParseIntError) -> Self {
        HexStringError::InvalidCharacter
    }
}

///////////////////////////////////////////////////////////////////////////////
// Enums
///////////////////////////////////////////////////////////////////////////////

/// Get/Set access rules. Announce is managed separately.
/// If not supported, then announce must be false.
#[derive(PartialEq, Eq, Debug)]
pub enum EpcAccessRule {
    /// The Get or Set operation is NOT supported
    NotSupported,
    /// The Get or Set operation is supported
    Supported,
    /// The Get or Set operation is mandatory. Implies supported.
    Mandatory,
}

/// Details of the a fault
#[derive(PartialEq, Debug, Display)]
#[repr(u16)]
pub enum NodeObjectFaultDescription {
    #[display("No error")]
    None = 0x0000,

    #[display("Recoverable (action: power cycle)")]
    RecoverableByPowerCycle = 0x0001,
    #[display("Recoverable (action: push reset)")]
    RecoverableByReset = 0x0002,
    #[display("Recoverable (action: physical adjustment)")]
    RecoverableByPhysicalAdjustment = 0x0003,
    #[display("Recoverable (action: add resources)")]
    RecoverableByAdditionalResources = 0x0004,
    #[display("Recoverable (action: cleaning required)")]
    RecoverableByCleaning = 0x0005,
    #[display("Recoverable (action: battery replacement)")]
    RecoverableBatteryReplacement = 0x0006,
    #[display("Recoverable (action: user defined, code: 0x{:04x})", _0)]
    RecoverableUserDefined(u8) = 0x0009,

    #[display("Repair (cause: safety tripped, index: 0x{:02x})", _0)]
    RepairSafetyDevice(u8) = 0x000a,
    #[display("Repair (cause: switch fault, index: 0x{:02x})", _0)]
    RepairSwitch(u8) = 0x0014,
    #[display("Repair (cause: sensor fault, index: 0x{:02x})", _0)]
    RepairSensor(u8) = 0x001e,
    #[display("Repair (cause: component fault, index: 0x{:02x})", _0)]
    RepairComponent(u8) = 0x003c,
    #[display("Repair (cause: control board, index: 0x{:02x})", _0)]
    RepairControlBoard(u8) = 0x005a,
    #[display("Repair (cause: user defined, code: 0x{:04x})", _0)]
    RepairUserDefined(u16) = 0x006f,

    #[display("Middleware failure (code: 0x{:04x})", _0)]
    EchonetMiddleware(u16) = 0x03e9,

    Indeterminate = 0x03ff
}

/// Simple conversion functions. The representation for this type is horrible.
impl NodeObjectFaultDescription {
    /// Create from a u16 value
    pub fn try_from_u16(value: u16) -> Result<Self, &'static str> {
        let low_byte = (0x00ff & value) as u8;
        let high_byte = ((0xff & value) >> 8) as u8;
        if (high_byte == 0x00 || high_byte >= 0x04) && low_byte < 0x6f {
            match low_byte {
                0x01 => Ok(NodeObjectFaultDescription::RecoverableByPowerCycle),
                0x02 => Ok(NodeObjectFaultDescription::RecoverableByReset),
                0x03 => Ok(NodeObjectFaultDescription::RecoverableByPhysicalAdjustment),
                0x04 => Ok(NodeObjectFaultDescription::RecoverableByAdditionalResources),
                0x05 => Ok(NodeObjectFaultDescription::RecoverableByCleaning),
                0x06 => Ok(NodeObjectFaultDescription::RecoverableBatteryReplacement),
                0x09 => Ok(NodeObjectFaultDescription::RecoverableUserDefined(high_byte)),
                x if x >= 0x0a && x <= 0x13 => Ok(NodeObjectFaultDescription::RepairSafetyDevice(low_byte - 0x0a)),
                x if x >= 0x14 && x <= 0x1d => Ok(NodeObjectFaultDescription::RepairSwitch(low_byte - 0x14)),
                x if x >= 0x1e && x <= 0x3b => Ok(NodeObjectFaultDescription::RepairSensor(low_byte - 0x1e)),
                x if x >= 0x3c && x <= 0x59 => Ok(NodeObjectFaultDescription::RepairComponent(low_byte - 0x3c)),
                x if x >= 0x5a && x <= 0x6e => Ok(NodeObjectFaultDescription::RepairControlBoard(low_byte - 0x5a)),
                _ => Err("Invalid value")
            }
        } else if value == 0x0000 {
            Ok(NodeObjectFaultDescription::None)
        } else if value >= 0x006f && value <= 0x03e8 {
            Ok(NodeObjectFaultDescription::RepairUserDefined(value))
        } else if value >= 0x03e9 && value <= 0x03ec {
            Ok(NodeObjectFaultDescription::EchonetMiddleware(value))
        } else if value == 0x03ff {
            Ok(NodeObjectFaultDescription::Indeterminate)
        } else {
            Err("Invalid value")
        }
    }

    /// Convert the struct to a u16
    pub fn to_u16(&self) -> u16 {
        match self {
            NodeObjectFaultDescription::None => 0x0000,
            NodeObjectFaultDescription::RecoverableByPowerCycle => 0x0001,
            NodeObjectFaultDescription::RecoverableByReset => 0x0002,
            NodeObjectFaultDescription::RecoverableByPhysicalAdjustment => 0x0003,
            NodeObjectFaultDescription::RecoverableByAdditionalResources => 0x0004,
            NodeObjectFaultDescription::RecoverableByCleaning => 0x0005,
            NodeObjectFaultDescription::RecoverableBatteryReplacement => 0x0006,
            NodeObjectFaultDescription::RecoverableUserDefined(high_byte) => (((*high_byte) as u16) << 8) | 0x0009,
            NodeObjectFaultDescription::RepairSafetyDevice(index) => 0x000a + ((*index) as u16),
            NodeObjectFaultDescription::RepairSwitch(index) => 0x0014 + ((*index) as u16),
            NodeObjectFaultDescription::RepairSensor(index) => 0x001e + ((*index) as u16),
            NodeObjectFaultDescription::RepairComponent(index) => 0x003c + ((*index) as u16),
            NodeObjectFaultDescription::RepairControlBoard(index) => 0x005a + ((*index) as u16),
            NodeObjectFaultDescription::RepairUserDefined(value) => *value,
            NodeObjectFaultDescription::EchonetMiddleware(value) => *value,
            NodeObjectFaultDescription::Indeterminate => 0x03ff
        }
    }
}
    
/// Allow conversion from a u16
impl TryFrom<u16> for NodeObjectFaultDescription {
    type Error = &'static str;

    fn try_from(v: u16) -> Result<Self, Self::Error> {
        NodeObjectFaultDescription::try_from_u16(v)
    }
}


#[derive(PartialEq, Debug, Display)]
pub enum NodeObjectInstallationLocation {
    LivingRoom(u8),
    DiningRoom(u8),
    Kitchen(u8),
    Bathroom(u8),
    Lavatory(u8),
    Washroom(u8),
    Passageway(u8),
    Room(u8),
    Stairway(u8),
    FrontDoor(u8),
    Storeroom(u8),
    Garden(u8),
    Garage(u8),
    Veranda(u8),
    Other(u8),
    UserDefined(u32),
    NotSpecified,
    Indefinite,
    #[display("Location (longitude: {_0}, latitude: {_1}, elevation: {_2})")]
    Location(f64, f64, f64),
    LocationInformationCode(u64)
}

/// Node capabilities
#[derive(PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum NodeType {
    General = super::NODE_TYPE_GENERAL,
    TransmitOnly = super::NODE_TYPE_TRANSMIT_ONLY
}

/// Node physical addresses
#[derive(PartialEq, Eq, Debug)]
pub enum NodePhysicalAddress {
    Localhost,
    IPv4(), // sock addr + interface, or ??
    IPV6(),
    Broadcast(), // Does not need an addr, it uses all.
    Serial(String),
}

///////////////////////////////////////////////////////////////////////////////
// Traits
///////////////////////////////////////////////////////////////////////////////

// A wrapper to hold all of the type implementations because.... rust.
pub trait EpcWrapper: Any + Debug {
    // Downcast
    fn as_any(&self) -> &dyn Any;
    // Downcast mutable
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

/// The trait desribing how all property types behave.
pub trait Epc
{
    // The associated type
    type Canonical: Debug + Display;

    /// Get the EPC code that this property implements
    fn epc(&self) -> u8;

    // Get the descriptive name of the EPC
    fn name(&self) -> &'static str;

    /// Accept the wire data
    /// 
    /// # Returns
    /// True if this property will accept the data pointed to by the buffer.
    fn accept(&self, wire: &[u8]) -> bool;

    /// Get the EPC property. Returns the raw property (i.e. the EDT, no EPC or PDC header)
    fn get(&self) -> Result<&[u8], EpcError>;
    
    /// Set the EPC property. Takes the raw property value (i.e. the EDT, no EPC or PDC header)
    fn set(&self, internal: &[u8]) -> Result<(), EpcError>;
    
    /// Get the value as the canonical type
    fn to_canonical(&self) -> Result<Self::Canonical, EpcError>;
    
    /// Set the value from a canonical type
    fn from_canonical(&self, canonical: &Self::Canonical) -> Result<(), EpcError>;

    /*
    /// Convert the property to a correctly formatted wire representation
    fn to_wire(&self) -> Result<Vec<u8>, EpcError>;
    /// Take a wire value and update the internal state from it.
    fn from_wire(&mut self, wire: &[u8]) -> Result<(), EpcError>;
    */

    /*
    fn get(&self) -> Result<std::cell::Ref<'_, Vec<u8>>, EpcError>;
    fn set(&self, internal: &[u8]) -> Result<(), EpcError>;


    /// Determine if this EPC required to announce of change. This only applied
    /// if the property is implemented.
    /// 
    /// # Returns
    /// True if the EPC is required to announce on change.
    fn announce(&self) -> bool;
    /// Is the EPC mandatory
    /// 
    /// # Returns
    /// True if either of the get or set access rules are mandatory.
    fn mandatory(&self) -> bool;

    /// Get the "Get" access rule.
    /// 
    /// If the value is [NodePropertyOperation::Mandatory], it implies that
    /// [NodePropertyOperation::Supported] is also set.
    /// 
    /// # Returns
    /// A [NodePropertyOperation] value depending on the access rule defined in the spec.
    fn get_get_access_rule(&self) -> NodePropertyOperation;

    /// Determine if the "Get" operation is actually implemented on the node
    /// 
    /// # Returns
    /// True if the node publishes that it supports the get operation.
    fn is_get_supported(&self) -> bool;

    /// Get the "Set" access rule
    /// 
    /// If the value is [NodePropertyOperation::Mandatory], it implies that
    /// [NodePropertyOperation::Supported] is also set.
    /// 
    /// # Returns
    /// A [NodePropertyOperation] value depending on the access rule defined in the spec.
    fn get_set_access_rule(&self) -> NodePropertyOperation;

    /// Determine if the "Set" operation is actually implemented on the node
    /// 
    /// # Returns
    /// True if the node publishes that it supports the get operation.
    fn is_set_supported(&self) -> bool;
    */
}

///////////////////////////////////////////////////////////////////////////////
// Types
///////////////////////////////////////////////////////////////////////////////

/// Basic Hex String implementation.
#[derive(Clone, Debug, Default, Display, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[display("{}", _0)]
#[repr(transparent)]
pub struct HexString(String);

/// Implementation for HexString
impl HexString {
    /// Constructor.
    /// 
    /// We will store excess padding on the front of the source string as this
    /// indicates how big the bytes are when converted. If there is an odd
    /// number of characters, it will assume there is a one character '0'
    /// pad.
    fn new(value: &str) -> Result<Self, HexStringError> {
        for c in value.chars() {
            if !((c >= '0' && c <= '9') || (c >= 'a' && c <= 'f') || (c >= 'A' && c <= 'F')) {
                return Err(HexStringError::InvalidCharacter);
            }
        }

        if value.len() % 2 == 0 {
            Ok(Self(value.to_owned()))
        } else {
            // Cheat, since we know the string is only ASCII characters, we can make some assumptions
            // that the number of bytes is the same as the number of chars.
            let mut dst = vec!['0' as u8; value.len()+1];
            (&mut dst[1..]).copy_from_slice(value.as_bytes());
            Ok(Self(unsafe {String::from_utf8_unchecked(dst) }))
        }
    }

    /// Create a HexString from a byte array
    /// 
    /// # Arguments
    ///
    /// * `buf` - The byte buffer to encode.
    /// * `len` - The total length of bytes to represent. If the length is
    ///           smaller than the buffer, and error is returned. If the
    ///           length is larger, the HexString will be left padded with
    ///           "00".
    pub fn from_bytes(buf: &[u8], len: usize) -> Result<Self, HexStringError> {
        let buf_start = if len < buf.len() {
            // Check each byte in the buffer to see if there is padding.
            // If there is, see if the value is small enough without the
            // padding.
            let mut pad_end = buf.len();
            for i in 0..buf.len() {
                if buf[i] != 0x00 {
                    pad_end = i;
                    break;
                }
            };
            if len < (buf.len() - pad_end) {
                return Err(HexStringError::BufferTooSmall);
            }
            pad_end
        } else {0};

        let mut encoded = String::with_capacity(len * 2);
        let pad_len = len - (buf.len() - buf_start);
        let pad_str = "00";
        for i in 0..len {
            if i < pad_len {
                encoded.push_str(pad_str);
            } else {
                let byte = &buf[i-pad_len+buf_start];
                write!(&mut encoded, "{:02x}", byte).expect("Unable to write");
            }
        }

        Ok(Self(encoded))
    }

    /// Decode the value into a new Vec
    pub fn decode(&self) -> Vec<u8> {
        let mut buf = vec![0x00_u8; self.0.len() / 2];
        self.decode_into_slice(buf.as_mut_slice()).unwrap();
        buf
    }

    /// Return the length of the actual string.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Return the number of bytes represented
    pub fn byte_len(&self) -> usize {
        self.0.len() / 2
    }

    /// Decode the value without checking the length.
    ///
    /// # Arguments
    ///
    /// `buf` - A mutable buffer to write the data into. The value will be
    ///         right aligned. The caller is required to make sure the size
    ///         of the slice is big enough the same size or larger than the
    ///         internal buffer length.
    /// 
    /// # Return
    /// 
    /// Nothing. Error on failure. 
    /// 
    /// Caller is expected to have allocated a buffer that is big enough.
    pub fn decode_into_slice(&self, buf: &mut [u8]) -> Result<(), HexStringError>{
        let internal_len: usize = self.0.len();
        let start_pos = if buf.len() * 2 < self.0.len() {
            // In this instance, see if there is enough zero padding to
            // put the value into the buf without losing information.
            let internal = self.0.as_bytes();
            let mut pad_end = buf.len();
            for i in (0..internal_len).step_by(2) {
                if internal[i] != '0' as u8 || internal[i+i] != '0' as u8 {
                    pad_end = i;
                    break;
                }
            };
            if buf.len() * 2 < (internal_len - pad_end) {
                return Err(HexStringError::BufferTooSmall);
            }
            pad_end
        } else {0};

        let buf_len: usize = buf.len();
        for i in 0..buf_len {
            let start = (internal_len as isize) - ((i as isize) * 2) - 2;
            // compare as isize as start may be less than zero.
            if start >= (start_pos as isize) {
                buf[buf_len - i - 1] = u8::from_str_radix(&self.0[(start as usize)..((start as usize)+2)], 16)?;
            } else {
                buf[buf_len - i - 1] = 0x00;
            }
        }

        Ok(())
    }
}

impl From<&[u8]> for HexString {
  fn from(bytes: &[u8]) -> Self {
    HexString::from_bytes(bytes, bytes.len()).unwrap()
  }
}

impl From<Vec<u8>> for HexString {
  fn from(bytes: Vec<u8>) -> Self {
    Self::from(&bytes[..])
  }
}

impl<const N: usize> From<[u8; N]> for HexString {
  fn from(bytes: [u8; N]) -> Self {
    Self::from(&bytes[..])
  }
}

/// Create a HexString from an existing string
impl FromStr for HexString {
  type Err = HexStringError;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    Self::new(s)
  }
}

/// Try to create from an existing owned String.
///
/// Try to avoid this it will make a duplicate copy of an already owned string.
impl TryFrom<String> for HexString {
    type Error = HexStringError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::new(&s)
    }
}

/// Convert the HexString into a byte array.
impl<const N: usize> TryFrom<HexString> for [u8; N] {
  type Error = HexStringError;

  fn try_from(s: HexString) -> Result<Self, Self::Error> {
    let mut bytes = [0u8; N];
    s.decode_into_slice(&mut bytes[..])?;
    Ok(bytes)
  }
}

/// Compare with a string
/// 
/// This will be a case insentivie string as an upper and lower case
/// hex string are canonically equal.
impl PartialEq<&str> for HexString {
    fn eq(&self, other: &&str) -> bool {
        self.0.to_ascii_lowercase() == (*other).to_ascii_lowercase()
    }
}

/// Holder for the version information and message types (profile object)
#[derive(Clone, Debug, Default, Display, Eq, From, Hash, Ord, PartialEq, PartialOrd)]
#[display("Profile support: ECHONET LITE {}.{}. Message types: specified: {}, arbiturary: {}", major_version, minor_version, specified_message, arbiturary_message)]
pub struct NodeProfileObjectEchonetLiteSupportedVersion {
    pub(in super) major_version: u8,
    pub(in super) minor_version: u8,
    pub specified_message: bool,
    pub arbiturary_message: bool,
}

impl NodeProfileObjectEchonetLiteSupportedVersion {
    pub fn version(&self) -> String {
        format!("{}.{}", self.major_version, self.minor_version)
    }
}

/// Holder for the version information and message types (device object)
#[derive(Clone, Debug, Default, Display, Eq, From, Hash, Ord, PartialEq, PartialOrd)]
#[display("Device suppport: release {} revision {}", release, revision)]
pub struct NodeDeviceObjectEchonetLiteSupportedVersion {
    pub release: char,
    pub revision: u8,
}

/// Holder for Unique Identifier Data
#[derive(Clone, Copy, Debug, Default, Display, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[display("{}", _0)]
#[repr(transparent)]
pub struct NodeObjectUniqueIdentifier(pub(in super)u16);

impl NodeObjectUniqueIdentifier {
    /// Is the 
    pub fn is_non_volatile(&self) -> bool {
        self.0 & 0x8000_u16 == 0
    }

    /// If not default, the node has had a value assigned by a system
    /// probably the controller. If performing a set operation, then
    /// the value the 
    pub fn is_default(&self) -> bool {
        self.0 & 0x4000_u16 == 0
    }

    // Change the value of the default flag.
    pub fn set_default(&mut self, value: bool) {
        self.0 = if value {
            self.0 & 0xbfff
        } else {
            self.0 | 0x4000
        };
    }
}

/// Holder for the supported EPC property maps
pub struct NodeObjectPropertyMap {
    /// byte 0xn0 + 0x80 operations
    operations: [u16; 8],
    operations_count: usize
}

impl NodeObjectPropertyMap {
    fn new() -> Self {
        NodeObjectPropertyMap{operations: [0x0000; 8], operations_count: 0}
    }

    /// Set the operation as enabled.
    /// 
    /// # Returns
    /// Ok(true) if the property was enabled, Ok(false) if not (already enabled)
    /// and Err if the operation was invalid.
    pub fn enable_operation(&mut self, operation: u8) -> Result<bool, MiddlewareError> {
        if !self.is_operation_enabled(operation)? {
            self.operations[((operation - 0x80) >> 4) as usize] |= 0x0001 << (operation & 0x0f);
            self.operations_count += 1;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Disable an operation
    /// 
    /// # Returns
    /// Ok(true) if the property was enabled, Ok(false) if not (already enabled)
    /// and Err if the operation was invalid.
    pub fn disable_operation(&mut self, operation: u8) -> Result<bool, MiddlewareError> {
        if self.is_operation_enabled(operation)? {
            self.operations[((operation - 0x80) >> 4) as usize] &= !(0x0001 << (operation & 0x0f));
            self.operations_count -= 1;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Disable all operations
    /// 
    /// # Returns
    /// Ok(true) if the property map was change, Ok(false) if not.
    pub fn disable_all(&mut self) -> Result<bool, MiddlewareError> {
        if self.operations_count > 0 {
            self.operations = [0x0000; 8];
            self.operations_count = 0;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Copy the set of operations from another instance
    pub fn copy_from(&mut self, other: &NodeObjectPropertyMap) {
        self.operations = other.operations;
        self.operations_count = other.operations_count;
    }

    /// Determines if an operation is enabled
    /// 
    /// # Arguments
    /// * `operation` - The operation to check (range 0x80 - 0xff inclusive)
    /// 
    /// # Returns
    /// Ok(true) if the operation is enabled, Ok(false) otherwise. Err if the
    /// operation value is invalid.
    pub fn is_operation_enabled(&self, operation: u8) -> Result<bool, MiddlewareError> {
        self.validate_operation(operation)?;
        Ok(unsafe {self.is_operation_enabled_unsafe(operation)})
    }

    /// Check if an operation is enabled without going through the proper checks.
    #[inline(always)]
    unsafe fn is_operation_enabled_unsafe(&self, operation: u8) -> bool {
        self.operations[((operation - 0x80) >> 4) as usize] & (0x0001 << (operation & 0x0f)) != 0
    }

    /// Validate the operation is within range.
    fn validate_operation(&self, operation: u8) -> Result<(), MiddlewareError> {
        if operation < 0x80 {
            return Err(MiddlewareError::InvalidValue("valid operations must be in the range 0x80 - 0xff inclusive".to_owned()));
        }
        Ok(())
    }

    /// Return the list of enabled operations as a list of u8
    pub fn as_operations_list(&self) -> Vec<u8> {
        let mut ops_list: Vec<u8> = Vec::with_capacity(self.operations_count);
        for i in 0x80..=0xff {
            if unsafe {self.is_operation_enabled_unsafe(i)} {
                ops_list.push(i);
            }
        }
        ops_list
    }

    /// Convert a byte format into a new NodeObjectPropertyMap
    pub fn from_bytes(wire: &[u8]) -> Result<Self, MiddlewareError> {
        if wire.len() == 0 || (wire[0] < 16 && ((wire[0] + 1) as usize != wire.len())) || (wire[0] >= 16 && wire.len() != 17) {
            return Err(MiddlewareError::InvalidValue("Invalid wire structure".to_owned()));
        }

        // Put in a copy and transfer after all validated to avoid leaving self in a broken state.
        let mut npm = NodeObjectPropertyMap::new();
        if wire[0] < 16 {
            // List decode. Using internal access to avoid validation overhead.
            for &operation in &wire[1..] {
                if operation < 0x80 {
                    return Err(MiddlewareError::InvalidValue(format!("Invalid operation '0x{:02x}' specified", operation)));
                }
                npm.operations[((operation & 0xf0) as usize - 0x80) >> 4] |= 0x0001 << (operation & 0x0f);
                npm.operations_count += 1;
            }

        } else {
            // Map decode. Using internal access to avoid validation overhead.
            let mut operations_count: usize = 0;
            for i in 0..8 {
                let mask = 0x01 << i;
                for j in (0..16).rev() {
                    npm.operations[i] = (npm.operations[i] << 1) | ((wire[j+1] & mask) >> i) as u16;
                }
                operations_count += npm.operations[i].count_ones() as usize;
            }

            if operations_count != wire[0] as usize {
                return Err(MiddlewareError::InvalidValue("Operation count mismatch in bitfield".to_owned()));
            }
            npm.operations_count = operations_count;
        }

        if wire[0] as usize == npm.operations_count {
            Ok(npm) 
        } else {
            Err(MiddlewareError::InvalidValue("Incorrect number of properties set".to_owned()))
        }
    }

    /// Convert to bytes (in the expected EPC wire format). This is not just an export of the internal state.
    /// 
    /// A tradeoff here. To have a more efficient conversion, we need to track the count,
    /// which means the set and clear operations are heavier.
    pub fn decode(&self) -> Vec<u8> {
        if self.operations_count < 16 {
            // 1 byte count + max 15 operations
            let mut wire = vec![0x00; self.operations_count + 1];
            wire[0] = self.operations_count as u8;
            let mut pos = 1;
            for i in 0..8 {
                for j in 0..16 {
                    if self.operations[i] & (0x0001 << j) != 0 {
                        wire[pos] = ((i << 4) | j) as u8 + 0x80;
                        pos += 1;
                    }
                }
            }
            wire
        } else {
            let mut wire = vec![0x00; 17];
            wire[0] = self.operations_count as u8;

            // Transpose. Would be quicker and easier with SIMD instructions, but that is not stable yet, and this is small
            for i in 0..8 {
                let mut bits = self.operations[i];
                for j in 0..16 {
                    wire[j+1] |= ((bits & 0x0001) as u8) << i;
                    bits >>= 1;
                }
            }

            wire
        }
    }
}

impl std::fmt::Debug for NodeObjectPropertyMap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut dbg = f.debug_struct("NodeObjectPropertyMap");
        
        dbg.field("operations_count", &self.operations_count);
        for i in 0..8 {
            let a = format!("{:02x}: {:08b} {:08b}", (i << 4) + 0x80, (self.operations[i] & 0xff00) >> 8, self.operations[i] & 0xff);
            dbg.field(format!("operation[{:02x}]", i).as_str(), &a);
        }

        dbg.finish()
    }
}

impl std::fmt::Display for NodeObjectPropertyMap {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "property map(operations={:?})", self.as_operations_list())
    }
}

/// Holder for the node instance count (hacky u24 type)
#[derive(Clone, Copy, Debug, Default, Display, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NodeObjectInstanceCount(pub(in super) u32);

/// ECHONET Lite Object Specification (in-node addressing)
/// A node can contain multiple objects which are addressable through the "ECHONET Lite Object Spefification" (EOJ)
/// * Device Objects. These contain state and properties as per "APPENDIX Detailed Requirements for ECHONET Device objects"
/// * Profile Objects. These define the device capabiltiies and pointers into the device objects
#[derive(Clone, Copy, Debug, Display)]
#[display("group: 0x{:02x} class: 0x{:02x} instance: 0x{:02x}", class_group_code, class_code, instance_code)]
#[repr(packed)]
pub struct EOJ {
    class_group_code: u8, // E.g. sensors, home equipment, etc
    class_code: u8, // The specific type, e.g. a presence sensor
    instance_code: u8 // The instance number of the presence sensor, for example devices that have both PIR and mmWave 
}

/// Implementation methods for EOJ
impl EOJ {
    pub fn from_groupclass_instance(group_class: &NodeGroupClass, instance: u8) -> Self {
        Self {
            class_group_code: group_class.class_group_code,
            class_code: group_class.class_code,
            instance_code: instance,
        }
    }

    /// Raw copy from an existing slice. Can do this because the struct is packed and only
    /// consists of u8 values.
    /// 
    /// This will create a new EOJ, without validating whether group and class
    pub unsafe fn from_bytes(buf: &[u8; std::mem::size_of::<Self>()]) -> Self {
        let mut uninit_struct = MaybeUninit::<EOJ>::uninit();
        unsafe {
            uninit_struct.as_mut_ptr().copy_from(buf.as_ptr().cast::<Self>(), 1);
            uninit_struct.assume_init()
        }
    }

    /// Get a reference to the struct as a u8 slice.
    pub fn as_bytes<'a>(&'a self) -> &'a [u8] {
        unsafe {
            std::slice::from_raw_parts(
                (self as *const EOJ) as *const u8,
                std::mem::size_of::<Self>(),
            )
        }
    }
}

/// Holder the group and class (first two bytes of the EOJ). This needs to be public as it contains
/// a displayable form of the EOJ (whereas the EOJ is more replated to the wire form)
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NodeGroupClass {
    pub class_group_code: u8, // E.g. sensors, home equipment, etc
    pub class_code: u8, // The specific type, e.g. a presence sensor
}

impl NodeGroupClass {
    /// Genernate a displayable version of the group/class
    pub fn to_display(&self) -> (&'static str, &'static str) {
        let exact = match self {
            &CLASS_CONTROL_CONTROLLER => Some((self.get_group_display_name().unwrap(), "Controller")),
            &CLASS_PROFILE_NODE_PROFILE => Some((self.get_group_display_name().unwrap(), "Node Profile")),
            _ => None
        };

        exact.or_else(|| Some( (self.get_group_display_name().or_else(||Some(UNKNOWN)).unwrap(), UNKNOWN)) ).unwrap()
    }

    /// Get the display name for a group
    fn get_group_display_name(&self) -> Option<&'static str> {
        match self.class_group_code {
            EOJ_CLASS_GROUP_SENSOR => Some("Sensor"),
            EOJ_CLASS_GROUP_AIRCON => Some("Air Conditioning"),
            EOJ_CLASS_GROUP_FACILITY => Some("Facility"),
            EOJ_CLASS_GROUP_HOUSEWORK => Some("Housework"),
            EOJ_CLASS_GROUP_HEALTH => Some("Health"),
            EOJ_CLASS_GROUP_CONTROL => Some("Control"),
            EOJ_CLASS_GROUP_AV => Some("Audio Visual"),
            EOJ_CLASS_GROUP_PROFILE => Some("Profile"),
            EOJ_CLASS_GROUP_USER => Some("User"),
            _ => None
        }
    }

}

impl std::fmt::Display for NodeGroupClass {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let (group_desc, class_desc) = self.to_display();
        write!(f, "Group '{}' Class '{}'", group_desc, class_desc)
    }
}

impl From<&EOJ> for NodeGroupClass {
    fn from(value: &EOJ) -> Self {
        Self {
            class_group_code: value.class_group_code,
            class_code: value.class_code
        }
    }
}

// Constants for different group/classes
/// Control class
pub const CLASS_CONTROL_CONTROLLER: NodeGroupClass = NodeGroupClass {class_group_code: EOJ_CLASS_GROUP_CONTROL, class_code: 0xff};
pub const CLASS_PROFILE_NODE_PROFILE: NodeGroupClass = NodeGroupClass {class_group_code: EOJ_CLASS_GROUP_PROFILE, class_code: 0xf0};
 