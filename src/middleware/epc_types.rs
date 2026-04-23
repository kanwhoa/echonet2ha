//! Module containing all of the usable EPC types.
use super::api::*;
use chrono::{Datelike, Timelike};
use core::f64;
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::fmt::{Debug, Display};
use std::{u8, u32};

#[cfg(test)]
mod tests;

///////////////////////////////////////////////////////////////////////////////
// EPC Constants
///////////////////////////////////////////////////////////////////////////////
pub const EPC_OPERATING_STATUS: u8 = 0x80;
pub const EPC_INSTALLATION_LOCATION: u8 = 0x81;
pub const EPC_VERSION_INFORMATION: u8 = 0x82;
pub const EPC_IDENTIFICATION_NUMBER: u8 = 0x83;
pub const EPC_MEASURED_INSTANTANEOUS_POWER_CONSUMPTION: u8 = 0x84;
pub const EPC_MEASURED_CUMULATIVE_POWER_CONSUMPTION: u8 = 0x85;
pub const EPC_MANUFACTURERS_FAULT_CODE: u8 = 0x86;
pub const EPC_CURRENT_LIMIT_SETTING: u8 = 0x87;
pub const EPC_FAULT_STATUS: u8 = 0x88;
pub const EPC_FAULT_CONTENT: u8 = 0x89;
pub const EPC_MANUFACTURER_CODE: u8 = 0x8a;
pub const EPC_BUSINESS_FACILITY_CODE: u8 = 0x8b;
pub const EPC_PRODUCT_CODE: u8 = 0x8c;
pub const EPC_SERIAL_NUMBER: u8 = 0x8d;
pub const EPC_PRODUCTION_DATE: u8 = 0x8e;
pub const EPC_POWER_SAVING: u8 = 0x8f;
pub const EPC_REMOTE_CONTROL: u8 = 0x93;
pub const EPC_CURRENT_TIME: u8 = 0x97;
pub const EPC_CURRENT_DATE: u8 = 0x98;
pub const EPC_POWER_LIMIT: u8 = 0x99;
pub const EPC_CUMULATIVE_OPERATING_TIME: u8 = 0x9a;
pub const EPC_ANNOUNCEMENT_PROPERTY_MAP: u8 = 0x9d;
pub const EPC_SET_PROPERTY_MAP: u8 = 0x9e;
pub const EPC_GET_PROPERTY_MAP: u8 = 0x9f;
pub const EPC_UNIQUE_IDENTIFIER_DATA: u8 = 0xbf;
pub const EPC_NUMBER_OF_SELFNODE_INSTANCES: u8 = 0xd3;
pub const EPC_NUMBER_OF_SELFNODE_CLASSES: u8 = 0xd4;
pub const EPC_INSTANCE_LIST_NOTIFICATION: u8 = 0xd5;
pub const EPC_SELFNODE_INSTANCE_LIST_S: u8 = 0xd6;
pub const EPC_SELFNODE_CLASS_LIST_S: u8 = 0xd7;

///////////////////////////////////////////////////////////////////////////////
// Serialisation/Deserialisation constants
///////////////////////////////////////////////////////////////////////////////
const EPC_OPERATION_STATUS_ON: u8 = 0x30;
const EPC_OPERATION_STATUS_OFF: u8 = 0x31;

// What message types are supported
const NODE_MESSAGE_FORMAT_SPECIFIED: u8 = 0x01;
const NODE_MESSAGE_FORMAT_ARBITRARY: u8 = 0x02;

// This is dumb rust, these are absolutely known at compile time...
macro_rules! ERR_MSG_INVALID_BOOLEAN {() => ("did not match true or false value");}
macro_rules! ERR_INVALID_LENGTH {() => ("Invalid length, expected {} bytes, found {}");}
macro_rules! ERR_INTEGER_UNDERFLOW {() => ("Integer underflow");}
macro_rules! ERR_INTEGER_OVERFLOW {() => ("Integer overflow");}

///////////////////////////////////////////////////////////////////////////////
// Types
///////////////////////////////////////////////////////////////////////////////
/// Converter function from canonical to internal
pub type FromCanonicalType<T> = Box<dyn Fn(&dyn Epc<Canonical = T>, &T) -> Result<Vec<u8>, EpcError>>;
/// Converter function from internal to canonical
pub type ToCanonicalType<T> = Box<dyn Fn(&dyn Epc<Canonical = T>, &[u8]) -> Result<T, EpcError>>;
/// Validator for internal values, i.e. when recieved from the wire.
pub type ValidatorType<T> = Box<dyn Fn(&dyn Epc<Canonical = T>, &[u8]) -> Result<bool, EpcError>>;

///////////////////////////////////////////////////////////////////////////////
// Enums
///////////////////////////////////////////////////////////////////////////////

/// The specification source. Storing this as a bitfield since it's not really
/// needed for runtime operations.
const EPC_DOCSOURCE_DEVICE_MASK: u16 = 0x00ff;
const EPC_DOCSOURCE_DEVICE_NONE: u16 = 0x0000;
const EPC_DOCSOURCE_DEVICE_SUPERCLASS: u16 = 0x0001;
const EPC_DOCSOURCE_DEVICE_CLASS: u16 = 0x0002;
const EPC_DOCSOURCE_PROFILE_MASK: u16 = 0xff00;
const EPC_DOCSOURCE_PROFILE_NONE: u16 = 0x0000;
const EPC_DOCSOURCE_PROFILE_SUPERCLASS: u16 = 0x0100;
const EPC_DOCSOURCE_PROFILE_CLASS: u16 = 0x0200;

///////////////////////////////////////////////////////////////////////////////
// EPC implementations
///////////////////////////////////////////////////////////////////////////////

/// Struct for static data properties.
pub(super) struct StaticNodeProperty<T>
{
    /// Name of the property. Forcing static strings to avoid lifetime issues.
    name: &'static str,

    /// The EPC code it is preresented by
    epc: u8,

    // The definition source in the spec
    source: u16,

    /// If the property is mandatory (i.e. must exist). Specific operations can also have mandatory values
    mandatory: bool,

    /// If the property requires an announce message on change
    announce: bool,

    /// If the property is Get/Set-able. This is used to control whether a message from Home Assistant will generate
    /// a ECHONET Lite message. I.e. if Home Assistant does a get, and that is not supported, then it will not create
    /// a network message. If the Home Assistant does a set, but the value is not settable, then it will result in an
    /// error to Home Assistant not a network message.
    /// See the NODE_VALUE_OPERATION_ constants.
    operations: Cell<u8>,

    /// The last known value. Only used for reference, and never an authoritative response.
    /// We will always store in the network format since it is ready to be used immediately.
    /// To avoid copying the data multiple times, the value needs to include two extra bytes
    /// on the vec, for the EPC and PDC. The converter does not need to set these as the
    /// the from_canonical/from_internal will set these appropriately based on the conveter's
    /// output.
    last_value: RefCell<Vec<u8>>,

    /// When the value was last updated (get or set) in ms from the epoch at UTC.
    /// A last updated value of 0 means that the value has never been updated, and hence the value is
    /// the default. No issue for Set. Get should return error.
    last_updated: Cell<i64>,

    /// Convert a canonical value to internal,
    from_canonical_fn: FromCanonicalType<T>,

    /// Convert the internal value to canonical,
    to_canonical_fn: ToCanonicalType<T>,

    /// Validate an internal value. The full EPC wire value will be passed. The EPC and PDC will be
    /// validated before passing to the validator, there is no need to re-check.
    validator_fn: ValidatorType<T>,
}

impl<T> std::fmt::Debug for StaticNodeProperty<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut dbg = f.debug_struct("StaticNodeProperty");

        dbg.field("name", &(self.name))
            .field("epc", &(self.epc));

        let ts = self.last_updated.get();
        if ts == 0 {
            dbg.field("last set", &"never");
        } else {
            let dt = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(ts).unwrap();
            let pdt = dt.to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
            dbg.field("last set", &pdt);
        }

        dbg.finish()
    }
}

/// General node property implementation
impl<T> StaticNodeProperty<T>
where
    T: Debug + Display
{
    /// Constructor for general type
    pub const fn new(name: &'static str, epc: u8, source: u16,
        announce: bool, get_operation: EpcAccessRule, set_operation: EpcAccessRule,
        from_canonical: FromCanonicalType<T>, to_canonical: ToCanonicalType<T>, validator: ValidatorType<T>) -> Self
    {
        Self {
            name: name,
            epc: epc,
            source: source,
            mandatory: match get_operation {
                EpcAccessRule::Mandatory => true,
                _ => false
            } || match set_operation {
                EpcAccessRule::Mandatory => true,
                _ => false
            },
            announce: announce,
            operations: Cell::new(match get_operation {
                EpcAccessRule::NotSupported => 0x00,
                EpcAccessRule::Supported => super::NODE_VALUE_OPERATION_GET_SUPPORTED,
                EpcAccessRule::Mandatory => super::NODE_VALUE_OPERATION_GET_MANDATORY | super::NODE_VALUE_OPERATION_GET_SUPPORTED,
            } | match set_operation {
                EpcAccessRule::NotSupported => 0x00,
                EpcAccessRule::Supported => super::NODE_VALUE_OPERATION_SET_SUPPORTED,
                EpcAccessRule::Mandatory => super::NODE_VALUE_OPERATION_SET_MANDATORY | super::NODE_VALUE_OPERATION_SET_SUPPORTED,
            }),
            last_value: RefCell::new(Vec::new()),
            last_updated: Cell::new(0),
            from_canonical_fn: from_canonical,
            to_canonical_fn: to_canonical,
            validator_fn: validator
        }
    }

    /// Convert the internal value to a canonical value
    pub fn to_canonical(&self) -> Result<T, EpcError> {
        if self.last_updated.get() == 0 {
            Err(EpcError::NoValue(self.epc))
        } else {
            (self.to_canonical_fn)(self, self.last_value.borrow().as_slice())
        }
    }
    
    /// Convert a canonical value into an internal value
    pub fn from_canonical(&self, canonical: &T) -> Result<(), EpcError> {
        let mut internal = (self.from_canonical_fn)(self, canonical)?;
        if internal.len() < 2 {
            Err(EpcError::InvalidValue(self.epc, "internal buffer length is required to be at least 2".to_owned()))
        } else {
            internal[0] = self.epc;
            // Catch an overflow. This needs to be handled cleanly. If there is an overflow, we are not shortening
            // the buffer, so we can still convert back to the canonical form. However, when this is sent over the
            // wire, the buffer needs to be truncated else the message will get corrupted.
            let data_len = internal.len() - 2;
            internal[1] = if data_len > 0xff {0xff} else {data_len as u8}; 

            if (self.validator_fn)(self, internal.as_slice())? {
                let mut cell_value = self.last_value.borrow_mut();
                *cell_value = internal;
                self.last_updated.set(chrono::Utc::now().timestamp_millis());
                Ok(())
            } else {
                Err(EpcError::ValidationFailed(self.epc))
            }
        }
    }

    /// Get the internal value. This will not include the EPC or PDC header.
    /// This will return a copy of the internal data. Too many borrowing and temp value
    /// errors in returning a ref to a slice.
    /// To get the wire read format, use to_wire().
    pub fn get(&self) -> Result<Vec<u8>, EpcError> {
        if self.last_updated.get() == 0 {
            Err(EpcError::NoValue(self.epc))
        } else {
            self.last_value.try_borrow()
                .map_err(|err| EpcError::ValueError(self.epc, format!("unable to borrow value: {}", err)))
                .map(|val| (&val[2..]).to_vec())
        } 
    }
    
    /// Set the internal value. Do not provide the EPC or PDC header. This will take a copy of the data
    /// since the actual buffer needs have the headers.
    /// To set from the wire format, use from_wire().
    pub fn set(&self, internal: &[u8]) -> Result<(), EpcError> {
        // Need to do a copy and extend the array
        let mut buf = vec![0x00_u8; internal.len() + 2];
        buf[0] = self.epc;
        
        // Catch an overflow. This needs to be handled cleanly. If there is an overflow, we are not shortening
        // the buffer, so we can still convert back to the canonical form. However, when this is sent over the
        // wire, the buffer needs to be truncated else the message will get corrupted.
        let data_len = internal.len();
        buf[1] = if data_len > 0xff {0xff} else {data_len as u8};

        // Copy and validate before actually setting
        (&mut buf[2..]).copy_from_slice(internal);
        (self.validator_fn)(self, buf.as_slice())?;

        // Set
        let mut cell_value = self.last_value.borrow_mut();
        *cell_value = buf;
        self.last_updated.set(chrono::Utc::now().timestamp_millis());
        Ok(())
    }

    /*
    /// Clone an existing property and set the default value from a canonical value
    fn clone_with_canonical_value(&self, value: &T) -> Result<StaticNodeProperty<T>, EpcError> {
        let internal = (self.from_canonical)(self, value)?;
        self.clone_with_internal_value(&internal)
    }

    /// Clone an existing property and set the default value from an internal value
    /// Will create a copy of the vec.
    fn clone_with_internal_value(&self, internal: &[u8]) -> Result<StaticNodeProperty<T>, EpcError> {
        let cloned = self.clone();
        cloned.set_internal(internal)?;
        Ok(cloned)
    }
    */
}

/// Wrapper with downcast. This is so we can store in a vec.
impl<T: 'static> EpcWrapper for StaticNodeProperty<T>{
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// EPC impl for StaticNodeProperty
impl<T> Epc for StaticNodeProperty<T>
where
    T: Debug + Display
{
    type Canonical = T;

    /// See [EPC::epc]
    fn epc(&self) -> u8 {
        self.epc
    }

    /// See [EPC::name]
    fn name(&self) -> &'static str {
        self.name
    }
    
    /// See [EPC::accept_wire]
    fn accept(&self, wire: &[u8]) -> bool {
        wire.len() >= 2 && wire[0] == self.epc
    }
    
    /// Get the internal value. This will not include the EPC or PDC header
    fn get(&self) -> Result<&[u8], EpcError> {
        todo!()
    }
    
    /// Set the internal value. Do not provide the EPC or PDC header
    fn set(&self, internal: &[u8]) -> Result<(), EpcError> {
        self.set(internal)
    }
    
    fn to_canonical(&self) -> Result<Self::Canonical, EpcError> {
        self.to_canonical()
    }
    
    fn from_canonical(&self, canonical: &Self::Canonical) -> Result<(), EpcError> {
        self.from_canonical(canonical)
    }

    /*
    fn announce(&self) -> bool {
        todo!()
    }
    
    fn mandatory(&self) -> bool {
        todo!()
    }
    
    fn get_get_access_rule(&self) -> NodePropertyOperation {
        todo!()
    }
    
    fn is_get_supported(&self) -> bool {
        todo!()
    }
    
    fn get_set_access_rule(&self) -> NodePropertyOperation {
        todo!()
    }
    
    fn is_set_supported(&self) -> bool {
        todo!()
    }
    */
}

///////////////////////////////////////////////////////////////////////////////
// Factory methods for creation of all EPCs (static variants only)
///////////////////////////////////////////////////////////////////////////////

/// Construct a standard boolean type
fn boolean_property(name: &'static str, epc: u8, source: u16,
    announce: bool, get_operation: EpcAccessRule, set_operation: EpcAccessRule,
    true_value: u8, false_value: u8) -> Result<Box<dyn EpcWrapper>, EpcError>
{
    general_property(
        name,
        epc,
        source,
        announce,
        get_operation,
        set_operation,
        move |_, canonical| {
            let mut internal = vec![0x00; 3];
            internal[2] = if *canonical {true_value} else {false_value};
            Ok(internal)
        },
        move |epc: &dyn Epc<Canonical = bool>, internal|
            if internal[2] == true_value {
                Ok(true)
            } else if internal[2] == false_value {
                Ok(false)
            } else {
                Err(EpcError::InvalidValue(epc.epc(), ERR_MSG_INVALID_BOOLEAN!().to_owned()))
            },
        move |epc: &dyn Epc<Canonical = bool>, internal|
            if internal.len() != 3 {
                Err(EpcError::InvalidValue(epc.epc(), format!(ERR_INVALID_LENGTH!(), 3, internal.len())))
            } else if internal[0] == true_value || internal[0] == false_value {
                return Ok(true)
            } else {
                Err(EpcError::InvalidValue(epc.epc(), ERR_MSG_INVALID_BOOLEAN!().to_owned()))
            }
    )
}

/// General handler for properties that should be treated as hex strings.
fn hex_string_property(name: &'static str, epc: u8, source: u16,
    announce: bool, get_operation: EpcAccessRule, set_operation: EpcAccessRule,
    validator: impl Fn(&dyn Epc<Canonical = HexString>, &[u8]) -> Result<bool, EpcError> + 'static) -> Result<Box<dyn EpcWrapper>, EpcError>
{
    general_property(
        name,
        epc,
        source,
        announce,
        get_operation,
        set_operation,
        move |epc: &dyn Epc<Canonical = HexString>, canonical| {
            let mut internal = vec![0x00_u8; canonical.byte_len() + 2];
            match canonical.decode_into_slice(&mut internal[2..]) {
                Ok(_) => Ok(internal),
                Err(err) => Err(EpcError::InvalidValue(epc.epc(), err.to_string()))
            }
        },
        move |_: &dyn Epc<Canonical = HexString>, internal| Ok((&internal[2..]).into()),
        validator
    )
}

/// General handler for properties that should be treated as ascii strings.
fn ascii_string_property(name: &'static str, epc: u8, source: u16,
    announce: bool, get_operation: EpcAccessRule, set_operation: EpcAccessRule,
    validator: impl Fn(&dyn Epc<Canonical = String>, &[u8]) -> Result<bool, EpcError> + 'static) -> Result<Box<dyn EpcWrapper>, EpcError>
{
    general_property(
        name,
        epc,
        source,
        announce,
        get_operation,
        set_operation,
        move |epc: &dyn Epc<Canonical = String>, canonical|
            if canonical.is_ascii() {
                let mut internal = vec![0x00_u8; canonical.len() + 2];
                (&mut internal[2..]).copy_from_slice(canonical.as_bytes());
                Ok(internal)
            } else {
                Err(EpcError::InvalidValue(epc.epc(), "Invalid ASCII characters found".to_owned()))
            },
        move |_: &dyn Epc<Canonical = String>, internal|
            unsafe { Ok(String::from_utf8_unchecked((&internal[2..]).to_vec())) },
        move |epc, internal|
            if (&internal[2..]).iter().all(|&b| (b as char).is_ascii()) {
                validator(epc, internal)
            } else {
                Err(EpcError::InvalidValue(epc.epc(), "Invalid ASCII characters found".to_owned()))
            }
    )
}

/// Integer trait for constrianing the integer property type.
/// Overflow and underflow methods are purposely on buffers due to odd sized integers
/// and otherwise causing a to_canonical conversion from validate.
trait Integer {
    const SIZE: usize;
    fn to_be_bytes(self) -> [u8; Self::SIZE];
    fn from_be_bytes(bytes: [u8; Self::SIZE]) -> Self;
    fn is_underflow(bytes: [u8; Self::SIZE]) -> bool;
    fn is_overflow(bytes: [u8; Self::SIZE]) -> bool;
}

/// Allow generic u16 integers
impl Integer for u16 {
    const SIZE: usize = std::mem::size_of::<Self>();

    #[inline(always)]
    fn to_be_bytes(self) -> [u8; Self::SIZE] {
        self.to_be_bytes()
    }
    
    #[inline(always)]
    fn from_be_bytes(bytes: [u8; Self::SIZE]) -> Self {
        u16::from_be_bytes(bytes)
    }

    #[inline(always)]
    fn is_underflow(bytes: [u8; Self::SIZE]) -> bool {
        bytes == [0xff, 0xfe]
    }

    #[inline(always)]
    fn is_overflow(bytes: [u8; Self::SIZE]) -> bool {
        bytes == [0xff, 0xff]
    }
}

/// Allow generic u32 integers
impl Integer for u32 {
    const SIZE: usize = std::mem::size_of::<Self>();

    #[inline(always)]
    fn to_be_bytes(self) -> [u8; Self::SIZE] {
        self.to_be_bytes()
    }
    
    #[inline(always)]
    fn from_be_bytes(bytes: [u8; Self::SIZE]) -> Self {
        u32::from_be_bytes(bytes)
    }

    #[inline(always)]
    fn is_underflow(bytes: [u8; Self::SIZE]) -> bool {
        bytes == [0xff, 0xff, 0xff, 0xfe]
    }

    #[inline(always)]
    fn is_overflow(bytes: [u8; Self::SIZE]) -> bool {
        bytes == [0xff, 0xff, 0xff, 0xff]
    }
}

/// Allow NodeObjectUniqueIdentifier
impl Integer for NodeObjectUniqueIdentifier {
    const SIZE: usize = std::mem::size_of::<Self>();

    #[inline(always)]
    fn to_be_bytes(self) -> [u8; Self::SIZE] {
        self.0.to_be_bytes()
    }
    
    #[inline(always)]
    fn from_be_bytes(bytes: [u8; Self::SIZE]) -> Self {
        NodeObjectUniqueIdentifier(u16::from_be_bytes(bytes))
    }

    #[inline(always)]
    fn is_underflow(_: [u8; Self::SIZE]) -> bool {
        false
    }

    #[inline(always)]
    fn is_overflow(_: [u8; Self::SIZE]) -> bool {
        false
    }
}

// Allow NodeObjectInstanceCount. This is a hacky 3 byte integer.
impl Integer for NodeObjectInstanceCount {
    const SIZE: usize = 3;

    // WARNING: this will lose precision if the internal value is set higher than 0xffffff
    #[inline(always)]
    fn to_be_bytes(self) -> [u8; Self::SIZE] {
        if self.0 >= 0x00ffffff {
            // Set the overflow status
            [0xff_u8; Self::SIZE]
        } else {
            let mut buf = [0x00_u8; Self::SIZE];
            let bytes = self.0.to_be_bytes();
            let start = std::mem::size_of::<Self>() - Self::SIZE;
            (&mut buf[..]).copy_from_slice(&bytes[start..]);
            buf
        }
    }
    
    #[inline(always)]
    fn from_be_bytes(bytes: [u8; Self::SIZE]) -> Self {
        let mut buf = [0x00_u8; std::mem::size_of::<u32>()];
        let start = std::mem::size_of::<Self>() - Self::SIZE;
        (&mut buf[start..]).copy_from_slice(&bytes[..]);
        NodeObjectInstanceCount(u32::from_be_bytes(buf))
    }

    #[inline(always)]
    fn is_underflow(bytes: [u8; Self::SIZE]) -> bool {
        bytes == [0xff, 0xff, 0xfe]
    }

    #[inline(always)]
    fn is_overflow(bytes: [u8; Self::SIZE]) -> bool {
        bytes == [0xff, 0xff, 0xff]
    }
}

/// Simple wrapper for integer types where the network value is the same size as the canonical value. This allows a straight
/// passthrough.
fn integer_property<T>(name: &'static str, epc: u8, source: u16,
    announce: bool, get_operation: EpcAccessRule, set_operation: EpcAccessRule) -> Result<Box<dyn EpcWrapper>, EpcError>
where
    T: Integer + Display + Debug + Copy + 'static,
    [(); T::SIZE]: Sized
{
    integer_property_validated(
        name,
        epc,
        source,
        announce,
        get_operation,
        set_operation,
        move |_, _| Ok(true)
    )
}

/// Generic handler for integer type properties
fn integer_property_validated<T>(name: &'static str, epc: u8, source: u16,
    announce: bool, get_operation: EpcAccessRule, set_operation: EpcAccessRule,
    validator: impl Fn(&dyn Epc<Canonical = T>, &[u8]) -> Result<bool, EpcError> + 'static) -> Result<Box<dyn EpcWrapper>, EpcError>
where
    T: Integer + Display + Debug + Copy + 'static,
    [(); T::SIZE]: Sized
{
    general_property(
        name,
        epc,
        source,
        announce,
        get_operation,
        set_operation,
        move |_: &dyn Epc<Canonical = T>, canonical| {
            let mut internal = vec![0x00_u8; T::SIZE + 2];
            (&mut internal[2..]).copy_from_slice(&(canonical.to_be_bytes()));
            Ok(internal)
        },
        move |_: &dyn Epc<Canonical = T>, internal| {
            let buf: [u8; T::SIZE] = (&internal[2..]).try_into().unwrap();
            Ok(T::from_be_bytes(buf))
        },
        move |epc, internal|
            if internal.len() != T::SIZE + 2 {
                Err(EpcError::InvalidValue(epc.epc(), format!(ERR_INVALID_LENGTH!(), T::SIZE + 2, internal.len())))
            } else {
                let buf: [u8; T::SIZE] = (&internal[2..]).try_into().unwrap();
                if T::is_underflow(buf) {
                    Err(EpcError::InvalidValue(epc.epc(), ERR_INTEGER_UNDERFLOW!().to_owned()))
                } else if T::is_overflow(buf) {
                    Err(EpcError::InvalidValue(epc.epc(), ERR_INTEGER_OVERFLOW!().to_owned().to_owned()))
                } else {
                    validator(epc, internal)
                }
            }
    )
}

trait ValidUnsignedIntegerSizes<const N: usize> {}
impl ValidUnsignedIntegerSizes<1> for () {}
impl ValidUnsignedIntegerSizes<2> for () {}
impl ValidUnsignedIntegerSizes<4> for () {}

/// Types that are a float mapped into an integer property. `N` is the number of integer bytes.
/// Due to how the values are used, the validator only works on canonical values. This is a special case,
/// and causes the to_canonical to be invoked as part of the validation. A key limitation to this is that
/// it cannot hold negative values.
/// # Arguments
///
/// * `max_value` - The specified maximum value.
/// * `precision` - The nuumber of decimal places to store. E.g. 3 == 0.001
fn unsigned_integer_packed_float_property<const N: usize>(name: &'static str, epc: u8, source: u16,
    announce: bool, get_operation: EpcAccessRule, set_operation: EpcAccessRule,
    max_value: f64, precision: u8) -> Result<Box<dyn EpcWrapper>, EpcError>
where
    (): ValidUnsignedIntegerSizes<N>
{
    // Should really do this as a compile time check, but doing it here at least is a startup check.
    let divisor = 1.0_f64 / 10.0_f64.powi(precision as i32);
    let actual_max_value = (max_value / divisor).ceil();
    let type_max_value = match N {
                1 => (u8::MAX - 2) as f64,
                2 => (u16::MAX - 2) as f64,
                4 => (u32::MAX - 2) as f64,
                _ => unreachable!()
            };
    if actual_max_value > type_max_value {
        return Err(EpcError::ValueTooLarge(epc, "Specified maximum value exceeds the underlying type size".to_owned()))
    }

    general_property(
        name,
        epc,
        source,
        announce,
        get_operation,
        set_operation,
        move |epc: &dyn Epc<Canonical = f64>, canonical| {
            let val = ((*canonical) / divisor).round();            
            let mut buf: Vec<u8> = vec![0x00_u8; N + 2];

            if val == -0.00 || (val >= 0.0 && val <= actual_max_value) {
                match N {
                    1 => buf[2] = val as u8,
                    2 => (&mut buf[2..]).copy_from_slice((val as u16).to_be_bytes().as_slice()),
                    4 => (&mut buf[2..]).copy_from_slice((val as u32).to_be_bytes().as_slice()),
                    _ => unreachable!()
                }
                Ok(buf)
            } else if val.is_nan() {
                Err(EpcError::InvalidValue(epc.epc(), "Unable to represent NAN".to_owned()))
            } else if val.is_sign_negative() {
                log::debug!("EPC {:02x}: {}", epc.epc(), ERR_INTEGER_UNDERFLOW!());
                buf.fill(0xff);
                buf[N + 1] = 0xfe;
                Ok(buf)
            } else {
                log::debug!("EPC {:02x}: {}", epc.epc(), ERR_INTEGER_OVERFLOW!());
                buf.fill(0xff);
                Ok(buf)
            }
        },
        move |epc: &dyn Epc<Canonical = f64>, internal| {
            let mut underflow_slice = vec![0xff_u8; N];
            underflow_slice[N-1] = 0xfe;
            let overflow_slice = vec![0xff_u8; N];
            let value_slice = &internal[2..];

            if value_slice == underflow_slice {
                log::debug!("EPC {:02x}: {}", epc.epc(), ERR_INTEGER_UNDERFLOW!());
                Ok(f64::NEG_INFINITY)
            } else if value_slice == overflow_slice {
                log::debug!("EPC {:02x}: {}", epc.epc(), ERR_INTEGER_OVERFLOW!());
                Ok(f64::INFINITY)
            } else {
                // Conversion to integer first. This is pretty hacky. u32 is the biggest type
                // defined by ECHONET Lite. Length has been validated before, so can safely
                // ignore the result.
                let int_value = match N {
                    1 => value_slice[0] as u32,
                    2 => u16::from_be_bytes(value_slice.try_into().unwrap()) as u32,
                    4 => u32::from_be_bytes(value_slice.try_into().unwrap()),
                    _ => unreachable!()
                };
                Ok((int_value as f64) * divisor)
            }
        },
        move |epc, internal|
            if internal.len() != N + 2 {
                Err(EpcError::InvalidValue(epc.epc(), format!(ERR_INVALID_LENGTH!(), N + 2, internal.len())))
            } else {
                // Convert to an integer of size and do the comparison. This is a bit of a duplication, but
                // safer and quicker than converting to a float for comparison. Should really do the max
                // value check as a compile time optimisation.
                let value_slice = &internal[2..];
                let actual_value = match N {
                    1 => value_slice[0] as u32,
                    2 => u16::from_be_bytes(value_slice.try_into().unwrap()) as u32,
                    4 => u32::from_be_bytes(value_slice.try_into().unwrap()),
                    _ => unreachable!()
                };

                if actual_value <= actual_max_value as u32 {
                    Ok(true)
                } else {
                    let mut underflow_slice = vec![0xff_u8; N];
                    underflow_slice[N-1] = 0xfe;
                    let overflow_slice = vec![0xff_u8; N];

                    // Check if an overflow value
                    if value_slice == underflow_slice || value_slice == overflow_slice {
                        Ok(true)
                    } else {
                        Err(EpcError::ValueTooLarge(epc.epc(), "Value exceeds specified type maximum".to_owned()))
                    }
                }
            }
    )
}

/// General handler for date types
fn date_property(name: &'static str, epc: u8, source: u16,
    announce: bool, get_operation: EpcAccessRule, set_operation: EpcAccessRule,
    validator: impl Fn(&dyn Epc<Canonical = chrono::NaiveDate>, &[u8]) -> Result<bool, EpcError> + 'static) -> Result<Box<dyn EpcWrapper>, EpcError>
{
    general_property(
        name,
        epc,
        source,
        announce,
        get_operation,
        set_operation,
        move |_: &dyn Epc<Canonical = chrono::NaiveDate>, canonical| {
            let mut buf = vec![0x00; 6];
            (&mut buf[2..4]).copy_from_slice(&(canonical.year() as i16).to_be_bytes());
            buf[4] = canonical.month() as u8;
            buf[5] = canonical.day() as u8;
            Ok(buf)
        },
        move |epc: &dyn Epc<Canonical = chrono::NaiveDate>, internal| {
            let year = u16::from_be_bytes(internal[2..4].try_into().unwrap()) as i32;
            let month = internal[4] as u32;
            let day = internal[5] as u32;

            let maybe_canonical = chrono::NaiveDate::from_ymd_opt(year, month, day);
            if let Some(canonical) = maybe_canonical {
                Ok(canonical)
            } else {
                return Err(EpcError::InvalidValue(epc.epc(), "error parsing date".to_owned()));
            }
        },
        move |epc, internal|
            if internal.len() == 6 {
                validator(epc, internal)
            } else {
                Err(EpcError::InvalidValue(epc.epc(), format!(ERR_INVALID_LENGTH!(), 6, internal.len())))
            }
    )
}

/// General handler for time types
fn time_property(name: &'static str, epc: u8, source: u16,
    announce: bool, get_operation: EpcAccessRule, set_operation: EpcAccessRule,
    validator: impl Fn(&dyn Epc<Canonical = chrono::NaiveTime>, &[u8]) -> Result<bool, EpcError> + 'static) -> Result<Box<dyn EpcWrapper>, EpcError>
{
    general_property(
        name,
        epc,
        source,
        announce,
        get_operation,
        set_operation,
        move |_: &dyn Epc<Canonical = chrono::NaiveTime>, canonical| {
            let mut buf = vec![0x00; 4];
            buf[2] = canonical.hour() as u8;
            buf[3] = canonical.minute() as u8;
            Ok(buf)
        },
        move |epc: &dyn Epc<Canonical = chrono::NaiveTime>, internal| {
            let hour = internal[2] as u32;
            let minute = internal[3] as u32;

            let maybe_canonical = chrono::NaiveTime::from_hms_opt(hour, minute, 0);
            if let Some(canonical) = maybe_canonical {
                Ok(canonical)
            } else {
                return Err(EpcError::InvalidValue(epc.epc(), "error parsing time".to_owned()));
            }
        },
        move |epc, internal|
            if internal.len() == 4 {
                if internal[2] < 24 && internal[3] < 60 {
                    validator(epc, internal)
                } else {
                    Err(EpcError::InvalidValue(epc.epc(), "Time not in range".to_owned()))
                }
            } else {
                Err(EpcError::InvalidValue(epc.epc(), format!(ERR_INVALID_LENGTH!(), 6, internal.len())))
            }
    )
}

/// General handler for duration types (only really simple durations)
fn duration_property(name: &'static str, epc: u8, source: u16,
    announce: bool, get_operation: EpcAccessRule, set_operation: EpcAccessRule,
    validator: impl Fn(&dyn Epc<Canonical = chrono::TimeDelta>, &[u8]) -> Result<bool, EpcError> + 'static) -> Result<Box<dyn EpcWrapper>, EpcError>
{
    general_property(
        name,
        epc,
        source,
        announce,
        get_operation,
        set_operation,
        move |epc: &dyn Epc<Canonical = chrono::TimeDelta>, canonical| {
            // Need to decide the most appropriate value based on overflows. Try smallest first
            let mut internal = vec![0x00_u8; 7];
            if let amount = canonical.num_seconds() && amount <= u32::MAX as i64 {
                internal[2] = 0x41;
                (&mut internal[3..8]).copy_from_slice(amount.to_be_bytes().as_slice());
            } else if let amount = canonical.num_minutes() && amount <= u32::MAX as i64 {
                internal[2] = 0x42;
                (&mut internal[3..8]).copy_from_slice(amount.to_be_bytes().as_slice());
            } else if let amount = canonical.num_hours() && amount <= u32::MAX as i64 {
                internal[2] = 0x43;
                (&mut internal[3..8]).copy_from_slice(amount.to_be_bytes().as_slice());
            } else if let amount = canonical.num_days() && amount <= u32::MAX as i64 {
                internal[2] = 0x44;
                (&mut internal[3..8]).copy_from_slice(amount.to_be_bytes().as_slice());
            } else {
                return Err(EpcError::InvalidValue(epc.epc(), "TimeDelta overflows internal value".to_owned()))
            }
            Ok(internal)
        },
        move |_: &dyn Epc<Canonical = chrono::TimeDelta>, internal| {
            let amount = u32::from_be_bytes((&internal[3..]).try_into().unwrap()) as i64;
            let td = match internal[2] {
                0x41 => chrono::TimeDelta::seconds(amount),
                0x42 => chrono::TimeDelta::minutes(amount),
                0x43 => chrono::TimeDelta::hours(amount),
                0x44 => chrono::TimeDelta::days(amount),
                _ => unreachable!()
            };
            Ok(td)
        },
        move |epc, internal|
            if internal.len() == 7 {
                if internal[2] >= 0x41 && internal[2] <= 0x44 {
                    validator(epc, internal)
                } else {
                    Err(EpcError::InvalidValue(epc.epc(), "Not a vaslid duration type".to_owned()))
                }
            } else {
                Err(EpcError::InvalidValue(epc.epc(), format!(ERR_INVALID_LENGTH!(), 6, internal.len())))
            }
    )
}

/// Handler for NodePropertyMap types
fn property_map_property(name: &'static str, epc: u8, source: u16,
    announce: bool, get_operation: EpcAccessRule, set_operation: EpcAccessRule) -> Result<Box<dyn EpcWrapper>, EpcError>
{    
    general_property(
        name,
        epc,
        source,
        announce,
        get_operation,
        set_operation,
        move |_: &dyn Epc<Canonical = NodeObjectPropertyMap>, canonical|
            // FIXME: To avoid a copy we need to calculate the size of the buffer needed based on the type.
            // requires knoweldge of how the struct will decode
            Ok(canonical.decode()),
        move |epc: &dyn Epc<Canonical = NodeObjectPropertyMap>, internal| 
            match NodeObjectPropertyMap::from_bytes(&internal[2..]) {
                Ok(canonical) => Ok(canonical),
                Err(err) => Err(EpcError::InvalidValue(epc.epc(), format!("{}", err)))
            },
        move |epc, internal|
            if internal.len() > 2 && ((internal[1] < 16 && ((internal[1] + 2) as usize == internal.len())) || internal.len() == 17) {
                Ok(true)
            } else {
                Err(EpcError::InvalidValue(epc.epc(), "Invalid data length".to_owned()))
            }
    )
}

/// Another stupid thing to work around the typing system
#[derive(Clone, Debug)]
#[repr(transparent)]
struct EOJVec(Vec<EOJ>);

impl std::fmt::Display for EOJVec {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        // Access the inner vector using self.0
        write!(f, "[")?;
        for (i, item) in self.0.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}", item)?;
        }
        write!(f, "]")
    }
}

/// Handler for a list of EOJ bytes
fn eoj_array_property(name: &'static str, epc: u8, source: u16,
    announce: bool, get_operation: EpcAccessRule, set_operation: EpcAccessRule) -> Result<Box<dyn EpcWrapper>, EpcError>
{
    general_property(
        name,
        epc,
        source,
        announce,
        get_operation,
        set_operation,
        move |_: &dyn Epc<Canonical = EOJVec>, canonical| {
            // Since we know that the EOJ is a vec, and vec has an underlying continious buffer,
            // then just copy the canonical buffer into a new buffer.
            let src_ptr = canonical.0.as_ptr() as *const u8;
            let dst_len = canonical.0.len() * std::mem::size_of::<EOJ>();
            let mut dst: Vec<u8> = Vec::with_capacity(dst_len + 2);
            unsafe {
                dst.set_len(dst_len + 2);
                dst.as_mut_ptr().add(2).copy_from(src_ptr, dst_len);
            }
            Ok(dst)
        },
        move |_: &dyn Epc<Canonical = EOJVec>, internal| {
            // There might be a performance advantage in just doing a single copy, however
            // it's complicated due to the allocation, and sizing. Simpler just to set
            let eoj_count = (internal.len() - 2) / 3;
            let mut eojs: Vec<EOJ> = Vec::with_capacity(eoj_count);
            for i in 0..eoj_count {
                let eoj_bytes = &internal[(i + 2)..(i + 2 + std::mem::size_of::<EOJ>())];
                unsafe {
                    eojs.push(EOJ::from_bytes(eoj_bytes.try_into().unwrap()));
                }
            }
            Ok(EOJVec(eojs))
        },
        move |_, internal|
            // Simple validator. May decide we need to verify the EPC codes at a later time.
            Ok(internal.len() >= 2 && (internal.len() - 2) %3 == 0 && (internal[1] as usize) % 3 == 0)
    )
}

/// A simple length validator support function
fn validate_length(epc_code: u8, internal: &[u8], len: usize) -> Result<bool, EpcError> {
    if internal.len() == len + 2 {
        Ok(true)
    } else {
        Err(EpcError::InvalidValue(epc_code, format!(ERR_INVALID_LENGTH!(), len + 2, internal.len())))
    }
}

/// General properties where all properties need to be specified. This is a simple wrapper around property generation
/// to remove come boxing/verbosity.
fn general_property<T>(name: &'static str, epc: u8, source: u16,
    announce: bool, get_operation: EpcAccessRule, set_operation: EpcAccessRule,
    from_canonical: impl Fn(&dyn Epc<Canonical = T>, &T) -> Result<Vec<u8>, EpcError> + 'static,
    to_canonical: impl Fn(&dyn Epc<Canonical = T>, &[u8]) -> Result<T, EpcError> + 'static,
    validator: impl Fn(&dyn Epc<Canonical = T>, &[u8]) -> Result<bool, EpcError> + 'static) -> Result<Box<dyn EpcWrapper>, EpcError>
where
    T: Display + Debug + 'static,
{
    Ok(Box::new(StaticNodeProperty::new(
        name,
        epc,
        source,
        announce,
        get_operation,
        set_operation,
        Box::new(from_canonical),
        Box::new(to_canonical),
        Box::new(validator)
    )))
}


/// Profile object property factory for any given group/class and EPC
/// Need to seperate device and profile because some of the implementations change for a property.
pub fn property_factory(group_class: &NodeGroupClass, epc: u8) -> Result<Box<dyn EpcWrapper>, EpcError> {
    let is_profile = group_class.class_group_code == super::api::EOJ_CLASS_GROUP_PROFILE;

    let property = if epc >= 0x80 && epc < 0xa0 {
        // Shared by all classes
        match epc {
            // Operating status. True == ON, False == OFF
            EPC_OPERATING_STATUS => boolean_property(
                "Operating Status",
                epc,
                EPC_DOCSOURCE_DEVICE_SUPERCLASS | EPC_DOCSOURCE_PROFILE_CLASS,
                true,
                EpcAccessRule::Mandatory,
                EpcAccessRule::Supported,
                EPC_OPERATION_STATUS_ON,
                EPC_OPERATION_STATUS_OFF
            ),
            EPC_INSTALLATION_LOCATION => Ok(Box::new(StaticNodeProperty::new(
                "Installation Location",
                epc,
                EPC_DOCSOURCE_DEVICE_SUPERCLASS | EPC_DOCSOURCE_PROFILE_NONE,
                true,
                EpcAccessRule::Mandatory,
                EpcAccessRule::Mandatory,
                Box::new(move |epc: &dyn Epc<Canonical = NodeObjectInstallationLocation>, canonical |
                    match canonical {
                        NodeObjectInstallationLocation::LivingRoom(location_number) => Ok([0x00, 0x00, 0x08 | (location_number | 0x07)].to_vec()),
                        NodeObjectInstallationLocation::DiningRoom(location_number) => Ok([0x00, 0x00, 0x10 | (location_number | 0x07)].to_vec()),
                        NodeObjectInstallationLocation::Kitchen(location_number) => Ok([0x00, 0x00, 0x18 | (location_number | 0x07)].to_vec()),
                        NodeObjectInstallationLocation::Bathroom(location_number) => Ok([0x00, 0x00, 0x20 | (location_number | 0x07)].to_vec()),
                        NodeObjectInstallationLocation::Lavatory(location_number) => Ok([0x00, 0x00, 0x28 | (location_number | 0x07)].to_vec()),
                        NodeObjectInstallationLocation::Washroom(location_number) => Ok([0x00, 0x00, 0x30 | (location_number | 0x07)].to_vec()),
                        NodeObjectInstallationLocation::Passageway(location_number) => Ok([0x00, 0x00, 0x38 | (location_number | 0x07)].to_vec()),
                        NodeObjectInstallationLocation::Room(location_number) => Ok([0x00, 0x00, 0x40 | (location_number | 0x07)].to_vec()),
                        NodeObjectInstallationLocation::Stairway(location_number) => Ok([0x00, 0x00, 0x48 | (location_number | 0x07)].to_vec()),
                        NodeObjectInstallationLocation::FrontDoor(location_number) => Ok([0x00, 0x00, 0x50 | (location_number | 0x07)].to_vec()),
                        NodeObjectInstallationLocation::Storeroom(location_number) => Ok([0x00, 0x00, 0x58 | (location_number | 0x07)].to_vec()),
                        NodeObjectInstallationLocation::Garden(location_number) => Ok([0x00, 0x00, 0x60 | (location_number | 0x07)].to_vec()),
                        NodeObjectInstallationLocation::Garage(location_number) => Ok([0x00, 0x00, 0x68 | (location_number | 0x07)].to_vec()),
                        NodeObjectInstallationLocation::Veranda(location_number) => Ok([0x00, 0x00, 0x70 | (location_number | 0x07)].to_vec()),
                        NodeObjectInstallationLocation::Other(location_number) => Ok([0x00, 0x00, 0x78 | (location_number | 0x07)].to_vec()),
                        NodeObjectInstallationLocation::UserDefined(value) => Ok(value.to_be_bytes().to_vec()),
                        NodeObjectInstallationLocation::NotSpecified => Ok([0x00, 0x00, 0x00].to_vec()),
                        NodeObjectInstallationLocation::Indefinite => Ok([0x00, 0x00, 0xff].to_vec()),
                        NodeObjectInstallationLocation::Location(_, _, _) => {
                            log::warn!("Location information is not supported");
                            todo!()
                        },
                        NodeObjectInstallationLocation::LocationInformationCode(location_code) => {
                            todo!();
                            Ok([0x00, 0x00, 0x1B, 0x00, 0x00, 0x00, 0x00, 0x03].to_vec()) // FIXME
                        }
                    }
                ),
                Box::new(move |epc: &dyn Epc<Canonical = NodeObjectInstallationLocation>, internal| 
                    match internal[2] {
                        0x00 => Ok(NodeObjectInstallationLocation::NotSpecified),
                        0x01 => {
                            if &internal[3..12] == &[0x00, 0x00, 0x1B, 0x00, 0x00, 0x00, 0x00, 0x03] {
                                Ok(NodeObjectInstallationLocation::LocationInformationCode(0))
                            } else {
                                log::warn!("Location information is not supported");
                                todo!()
                            }
                        },
                        0xff => {
                            Ok(NodeObjectInstallationLocation::Indefinite)
                        }
                        _ => {
                            if internal[2] & 0x00_u8 == 0x00 {
                                let location_number = internal[2] & 0x07;
                                match internal[2] & 0x78 {
                                    0x08 => Ok(NodeObjectInstallationLocation::LivingRoom(location_number)),
                                    0x10 => Ok(NodeObjectInstallationLocation::DiningRoom(location_number)),
                                    0x18 => Ok(NodeObjectInstallationLocation::Kitchen(location_number)),
                                    0x20 => Ok(NodeObjectInstallationLocation::Bathroom(location_number)),
                                    0x28 => Ok(NodeObjectInstallationLocation::Lavatory(location_number)),
                                    0x30 => Ok(NodeObjectInstallationLocation::Washroom(location_number)),
                                    0x38 => Ok(NodeObjectInstallationLocation::Passageway(location_number)),
                                    0x40 => Ok(NodeObjectInstallationLocation::Room(location_number)),
                                    0x48 => Ok(NodeObjectInstallationLocation::Stairway(location_number)),
                                    0x50 => Ok(NodeObjectInstallationLocation::FrontDoor(location_number)),
                                    0x58 => Ok(NodeObjectInstallationLocation::Storeroom(location_number)),
                                    0x60 => Ok(NodeObjectInstallationLocation::Garden(location_number)),
                                    0x68 => Ok(NodeObjectInstallationLocation::Garage(location_number)),
                                    0x70 => Ok(NodeObjectInstallationLocation::Veranda(location_number)),
                                    0x78 => Ok(NodeObjectInstallationLocation::Other(location_number)),
                                    _ => unreachable!()
                                }
                            } else {
                                Ok(NodeObjectInstallationLocation::UserDefined((internal[2] & 0x7f_u8) as u32))
                            }
                        }
                    }
                ),
                Box::new(move |_: &dyn Epc<Canonical = NodeObjectInstallationLocation>, internal| {
                    Ok(internal.len() > 2 && (internal[0] == 0x01 && internal.len() == 17) || internal.len() == 3)
                })
            )) as Box<dyn EpcWrapper>),
            // This type is special. It contains the version supported AND the message type supported. However,
            // in ECHONET Lite, only the "specified message format" is supported, meaning that the if the device
            // advertisies "arbitrary message format", chances are we won't be able to interpret any messages from
            // the device. The actual message format is stored in EHD1/EHD2 headers. For here, we will just store
            // the value. This is complex because the device and profile types/uses differ significantly.
            EPC_VERSION_INFORMATION => if is_profile {
                general_property(
                    "Version Information",
                    epc,
                    EPC_DOCSOURCE_DEVICE_SUPERCLASS | EPC_DOCSOURCE_PROFILE_CLASS,
                    false,
                    EpcAccessRule::Mandatory,
                    EpcAccessRule::NotSupported,
                    move |np: &dyn Epc<Canonical = NodeProfileObjectEchonetLiteSupportedVersion>, canonical|
                        if canonical.specified_message || canonical.arbiturary_message {
                            let mut internal = vec![0x00_u8; 6];
                            internal[2] = canonical.major_version;
                            internal[3] = canonical.minor_version;
                            internal[4] = if canonical.specified_message {NODE_MESSAGE_FORMAT_SPECIFIED} else {0x00} | if canonical.arbiturary_message {NODE_MESSAGE_FORMAT_ARBITRARY} else {0x00};
                            Ok(internal)
                        } else {
                            return Err(EpcError::InvalidValue(np.epc(), "specified or arbitrary messages must be supported".to_owned()))
                        },
                    move |_, internal| Ok(NodeProfileObjectEchonetLiteSupportedVersion {
                        major_version: internal[2],
                        minor_version: internal[3],
                        specified_message: (internal[4] & NODE_MESSAGE_FORMAT_SPECIFIED) == NODE_MESSAGE_FORMAT_SPECIFIED,
                        arbiturary_message: (internal[4] & NODE_MESSAGE_FORMAT_ARBITRARY) == NODE_MESSAGE_FORMAT_ARBITRARY,
                    }),
                    move |epc, internal|
                        if internal.len() == 6 {
                            if (internal[4] & NODE_MESSAGE_FORMAT_SPECIFIED) == NODE_MESSAGE_FORMAT_SPECIFIED || (internal[4] & NODE_MESSAGE_FORMAT_ARBITRARY) == NODE_MESSAGE_FORMAT_ARBITRARY {
                                Ok(true)
                            } else {
                                Err(EpcError::InvalidValue(epc.epc(), "specified or arbitrary messages must be supported".to_owned()))
                            }
                        } else {
                            Err(EpcError::InvalidValue(epc.epc(), format!(ERR_INVALID_LENGTH!(), 6, internal.len())))
                        }
                )
            } else {
                general_property(
                    "Version Information",
                    epc,
                    EPC_DOCSOURCE_DEVICE_SUPERCLASS | EPC_DOCSOURCE_PROFILE_CLASS,
                    false,
                    EpcAccessRule::Mandatory,
                    EpcAccessRule::NotSupported,
                    move |_: &dyn Epc<Canonical = NodeDeviceObjectEchonetLiteSupportedVersion>, canonical| {
                        let mut internal = vec![0x00_u8; 6];
                        internal[4] = canonical.release.to_ascii_uppercase() as u8;
                        internal[5] = canonical.revision;
                        Ok(internal)
                    },
                    move |_, internal| Ok(NodeDeviceObjectEchonetLiteSupportedVersion {
                        release: internal[2] as char,
                        revision: internal[3],
                    }),
                    move |epc, internal|
                        if internal.len() == 6 {
                            if (internal[4] as char).is_ascii() {
                                Ok(true)
                            } else {
                                Err(EpcError::InvalidValue(epc.epc(), "invalid release".to_owned()))
                            }
                        } else {
                            Err(EpcError::InvalidValue(epc.epc(), format!(ERR_INVALID_LENGTH!(), 6, internal.len())))
                        }
                )
            },
            // This stores two values. The whole value can be considered the identification number, with the
            // first byte identifying the communication medium. In ECHONET Lite, this is fixed at 0xfe. This also means
            // that the rest of the data is the "manufacturer specified format". This is basically 3 bytes identifying
            // the manufacturer and then the remaining 13 bytes specified by the manufacturer.
            EPC_IDENTIFICATION_NUMBER => hex_string_property(
                "Identification number",
                epc,
                EPC_DOCSOURCE_DEVICE_SUPERCLASS | EPC_DOCSOURCE_PROFILE_CLASS,
                false,
                EpcAccessRule::Mandatory,
                EpcAccessRule::NotSupported,
                move |epc, internal| if internal.len() == 17 {
                    if is_profile {
                        if internal[0] == 0xfe {Ok(true)} else {Err(EpcError::InvalidValue(epc.epc(), "Invalid identifier class".to_owned()))}
                    } else {
                        if internal[0] == 0x00 || internal[0] == 0xfe || internal[0] == 0xff {Ok(true)} else {Err(EpcError::InvalidValue(epc.epc(), "Invalid identifier class".to_owned()))}
                    }
                } else {
                    Err(EpcError::InvalidValue(epc.epc(), format!("expected 17 bytes found {}", internal.len())))
                }
            ),
            EPC_MEASURED_INSTANTANEOUS_POWER_CONSUMPTION => integer_property::<u16>(
                "Measured Instantaneous Power Consumption (W)",
                epc,
                EPC_DOCSOURCE_DEVICE_SUPERCLASS | EPC_DOCSOURCE_PROFILE_NONE,
                false,
                EpcAccessRule::Supported,
                EpcAccessRule::NotSupported
            ),
            EPC_MEASURED_CUMULATIVE_POWER_CONSUMPTION => unsigned_integer_packed_float_property::<4>(
                "Measured Cumulative Power Consumption (kWh)",
                epc,
                EPC_DOCSOURCE_DEVICE_SUPERCLASS | EPC_DOCSOURCE_PROFILE_NONE,
                false,
                EpcAccessRule::Supported,
                EpcAccessRule::NotSupported,
                999_999.999,
                3
            ),
            EPC_MANUFACTURERS_FAULT_CODE => general_property(
                "Manufacturer's fault Code",
                epc,
                EPC_DOCSOURCE_DEVICE_SUPERCLASS | EPC_DOCSOURCE_PROFILE_NONE,
                false,
                EpcAccessRule::Supported,
                EpcAccessRule::NotSupported,
                move |epc: &dyn Epc<Canonical = NodeObjectManufacturerFaultCode>, canonical| {
                    let buf_len = canonical.manufacturer_code.byte_len() + canonical.fault_code.byte_len() + 2;
                    let mut buf = vec![0x00_u8; buf_len];
                    canonical.manufacturer_code.decode_into_slice(&mut buf[2..])
                        .map_err(|err| EpcError::InvalidValue(epc.epc(), err.to_string()))?;
                    canonical.fault_code.decode_into_slice(&mut buf[5..])
                        .map_err(|err| EpcError::InvalidValue(epc.epc(), err.to_string()))?;
                    Ok(buf)
                },
                move |epc: &dyn Epc<Canonical = NodeObjectManufacturerFaultCode>, internal| {
                    // Validator guarentees the manufacturer code will exist.
                    let manufacturer_code = HexString::from_bytes(&internal[2..5], 3)
                        .map_err(|err|EpcError::InvalidValue(epc.epc(), err.to_string()))?;
                    let fault_code = HexString::from_bytes(&internal[5..], internal.len() - 5)
                        .map_err(|err|EpcError::InvalidValue(epc.epc(), err.to_string()))?;
                    Ok(NodeObjectManufacturerFaultCode {manufacturer_code, fault_code})
                },
                move |epc, internal| {
                    if internal.len() >= 5 { // 2 header + 3 manufacturer code
                        Ok(true)
                    } else {
                        Err(EpcError::InvalidValue(epc.epc(), format!("Required length must be greater than {}, was {}", 5, internal.len())))
                    }
                }
            ),
            EPC_CURRENT_LIMIT_SETTING => unsigned_integer_packed_float_property::<1>(
                "Current Limit Setting (%)",
                epc,
                EPC_DOCSOURCE_DEVICE_SUPERCLASS | EPC_DOCSOURCE_PROFILE_NONE,
                false,
                EpcAccessRule::Supported,
                EpcAccessRule::Supported,
                100.0,
                0
            ),
            // Fault status. True if a fault is encountered
            EPC_FAULT_STATUS => boolean_property(
                "Fault Status",
                epc,
                EPC_DOCSOURCE_DEVICE_SUPERCLASS | EPC_DOCSOURCE_PROFILE_SUPERCLASS,
                if is_profile {false} else {true},
                EpcAccessRule::Supported,
                EpcAccessRule::NotSupported,
                0x41,
                0x42
            ),
            // Fault content: 0x0000 to 0x03E8 same as device. 0x03E9 to 0x03EC: abnormality codes of ECHONET Lite middleware
            // adapters described in "Part III, ECHONET Lite Communications Equipment Specifications."
            // This is sort of an integer, but if we use the integer technique, conversion errors will not be caught.
            EPC_FAULT_CONTENT => general_property(
                "Fault Content (Fault Description)",
                epc,
                EPC_DOCSOURCE_DEVICE_SUPERCLASS | EPC_DOCSOURCE_PROFILE_CLASS,
                false,
                EpcAccessRule::Supported,
                EpcAccessRule::NotSupported,
                move |epc: &dyn Epc<Canonical = NodeObjectFaultDescription>, canonical| {
                    let mut buf = vec![0x00_u8; std::mem::size_of::<u16>() + 2];
                    let bytes = canonical.to_u16().to_be_bytes();
                    (&mut buf[2..]).copy_from_slice(&bytes);
                    Ok(buf)
                },
                move |epc: &dyn Epc<Canonical = NodeObjectFaultDescription>, internal| {
                    // Safe as the validator already checked the size.
                    let val = u16::from_be_bytes((&internal[2..]).try_into().unwrap());
                    NodeObjectFaultDescription::try_from_u16(val)
                        .map_err(|msg| EpcError::InvalidValue(epc.epc(), msg.to_owned()))
                },
                move |epc, internal| {
                    // Only way to validate is by resolving the enum Check the size first.
                    validate_length(epc.epc(), internal, std::mem::size_of::<u16>())?;
                    epc.to_canonical().map(|_|true)
                }
            ),
            EPC_MANUFACTURER_CODE => hex_string_property(
                "Manufacturer Code",
                epc,
                EPC_DOCSOURCE_DEVICE_SUPERCLASS | EPC_DOCSOURCE_PROFILE_SUPERCLASS,
                false,
                EpcAccessRule::Mandatory,
                EpcAccessRule::NotSupported,
                move |epc, internal| validate_length(epc.epc(), internal, 3)
            ),
            EPC_BUSINESS_FACILITY_CODE => hex_string_property(
                "Business Facility Code (Place of Business Code)",
                epc,
                EPC_DOCSOURCE_DEVICE_SUPERCLASS | EPC_DOCSOURCE_PROFILE_SUPERCLASS,
                false,
                EpcAccessRule::Supported,
                EpcAccessRule::NotSupported,
                move |epc, internal| validate_length(epc.epc(), internal, 3)
            ),
            EPC_PRODUCT_CODE => ascii_string_property(
                "Product Code",
                epc,
                EPC_DOCSOURCE_DEVICE_SUPERCLASS | EPC_DOCSOURCE_PROFILE_SUPERCLASS,
                false,
                EpcAccessRule::Supported,
                EpcAccessRule::NotSupported,
                move |epc, internal| validate_length(epc.epc(), internal, 12)
            ),
            EPC_SERIAL_NUMBER => ascii_string_property(
                "Serial Number (Production Number)",
                epc,
                EPC_DOCSOURCE_DEVICE_SUPERCLASS | EPC_DOCSOURCE_PROFILE_SUPERCLASS,
                false,
                EpcAccessRule::Supported,
                EpcAccessRule::NotSupported,
                move |epc, internal| validate_length(epc.epc(), internal, 12)
            ),
            EPC_PRODUCTION_DATE => date_property(
                "Production Date (Date of Manufacture)",
                epc,
                EPC_DOCSOURCE_DEVICE_SUPERCLASS | EPC_DOCSOURCE_PROFILE_SUPERCLASS,
                false,
                EpcAccessRule::Supported,
                EpcAccessRule::NotSupported,
                move |_, _| Ok(true)
            ),
            EPC_POWER_SAVING => boolean_property(
                "Power Saving",
                epc,
                EPC_DOCSOURCE_DEVICE_SUPERCLASS | EPC_DOCSOURCE_PROFILE_NONE,
                false,
                EpcAccessRule::Supported,
                EpcAccessRule::Supported,
                0x41,
                0x42
            ),
            // Remote control. This is a horrible property, with a very badly worded description.
            EPC_REMOTE_CONTROL => general_property(
                "Remote Control",
                epc,
                EPC_DOCSOURCE_DEVICE_SUPERCLASS | EPC_DOCSOURCE_PROFILE_NONE,
                false,
                EpcAccessRule::Supported,
                EpcAccessRule::Supported,
                move |_: &dyn Epc<Canonical = NodeObjectRemoteControl>, canonical| {
                    let mut buf = vec![0x00_u8; 3];
                    buf[2] = if canonical.network_type == NetworkType::NonPublic {0x41} else {0x42} | 
                        if canonical.network_status == NetworkStatus::Ok {0x60} else {0x00};
                    Ok(buf)
                },
                move |_: &dyn Epc<Canonical = NodeObjectRemoteControl>, internal| {
                    let network_type = if internal[2] & 0x41 == 0x41 {NetworkType::NonPublic} else {NetworkType::Public};
                    let network_status = if internal[2] & 0x60 == 0x60 {NetworkStatus::Ok} else {NetworkStatus::NotOk};
                    Ok(NodeObjectRemoteControl {network_type, network_status})
                },
                move |epc, internal| {
                    // Only way to validate is by resolving the enum Check the size first.
                    validate_length(epc.epc(), internal, 1)?;
                    if internal[2] & 0x40 == 0x40 && internal[2] & 0x03 != 0x00 {
                        Ok(true)
                    } else {
                        Err(EpcError::InvalidValue(epc.epc(), "Invalid boolean value".to_owned()))
                    }
                }
            ),
            EPC_CURRENT_TIME => time_property(
                "Current Time",
                epc,
                EPC_DOCSOURCE_DEVICE_SUPERCLASS | EPC_DOCSOURCE_PROFILE_NONE,
                false,
                EpcAccessRule::Supported,
                EpcAccessRule::Supported,
                move |_, _| Ok(true)
            ),
            EPC_CURRENT_DATE => date_property(
                "Current Date",
                epc,
                EPC_DOCSOURCE_DEVICE_SUPERCLASS | EPC_DOCSOURCE_PROFILE_NONE,
                false,
                EpcAccessRule::Supported,
                EpcAccessRule::Supported,
                move |_, _| Ok(true)
            ),
            EPC_POWER_LIMIT => integer_property::<u16>(
                "Power Limit (W)",
                epc,
                EPC_DOCSOURCE_DEVICE_SUPERCLASS | EPC_DOCSOURCE_PROFILE_NONE,
                false,
                EpcAccessRule::Supported,
                EpcAccessRule::Supported
            ),
            EPC_CUMULATIVE_OPERATING_TIME => duration_property(
                "Cumulative Operating Time",
                epc,
                EPC_DOCSOURCE_DEVICE_SUPERCLASS | EPC_DOCSOURCE_PROFILE_NONE,
                false,
                EpcAccessRule::Supported,
                EpcAccessRule::NotSupported,
                move |_, _| Ok(true)
            ),
            // These are dynamic for the self node, but static for other nodes.
            EPC_ANNOUNCEMENT_PROPERTY_MAP => property_map_property(
                "Status change announcement property map",
                epc,
                EPC_DOCSOURCE_DEVICE_SUPERCLASS | EPC_DOCSOURCE_PROFILE_SUPERCLASS,
                false,
                EpcAccessRule::Mandatory,
                EpcAccessRule::NotSupported,
            ),
            EPC_SET_PROPERTY_MAP => property_map_property(
                "Set operation property map",
                epc,
                EPC_DOCSOURCE_DEVICE_SUPERCLASS | EPC_DOCSOURCE_PROFILE_SUPERCLASS,
                false,
                EpcAccessRule::Mandatory,
                EpcAccessRule::NotSupported,
            ),
            EPC_GET_PROPERTY_MAP => property_map_property(
                "Get operation property map",
                epc,
                EPC_DOCSOURCE_DEVICE_SUPERCLASS | EPC_DOCSOURCE_PROFILE_SUPERCLASS,
                false,
                EpcAccessRule::Mandatory,
                EpcAccessRule::NotSupported,
            ),
            _ => Err(EpcError::NotAvailable(epc))
        }
    } else if epc >= 0xa0 && epc < 0xf0 {
        // Should be group specific, but maybe not. Try group first.
        // Create a function to check the group, if not, try the class.
        // group epc >= 0xa0 && epc < 0xc0
        // class epc >= 0xc0 && epc < 0xf0
        match group_class {
            &CLASS_PROFILE_NODE_PROFILE => property_factory_class_profile(epc),
            _ => Err(EpcError::NotAvailable(epc))
        }
    } else {
        // User defined area.
        Err(EpcError::NotAvailable(epc))
    }?;

    // Configure the default
    Ok(property)
}

/// Create the properties for the node profile
fn property_factory_class_profile(epc: u8) -> Result<Box<dyn EpcWrapper>, EpcError> {
    match epc {
        EPC_UNIQUE_IDENTIFIER_DATA => integer_property::<NodeObjectUniqueIdentifier>(
            "Unique Identifier Data",
            epc,
            EPC_DOCSOURCE_DEVICE_NONE | EPC_DOCSOURCE_PROFILE_CLASS,
            false,
            EpcAccessRule::Supported,
            EpcAccessRule::Supported
        ),
        EPC_NUMBER_OF_SELFNODE_INSTANCES => integer_property::<NodeObjectInstanceCount>(
            "Number of Self-Node Instances",
            epc,
            EPC_DOCSOURCE_DEVICE_NONE | EPC_DOCSOURCE_PROFILE_CLASS,
            false,
            EpcAccessRule::Mandatory,
            EpcAccessRule::NotSupported
        ),
        EPC_NUMBER_OF_SELFNODE_CLASSES => integer_property::<u16>(
            "Number of Self-Node Classes",
            epc,
            EPC_DOCSOURCE_DEVICE_NONE | EPC_DOCSOURCE_PROFILE_CLASS,
            false,
            EpcAccessRule::Mandatory,
            EpcAccessRule::NotSupported
        ),
        // Does not include Node Profile objects. Max 84. If over this, then the announcement message can have multiple
        // EPC blocks, with the same OPC code, i.e. a repeated OPC. However, that is difficult to interpret, as multiple
        // OPC valus in the same message need to be treated as a logical and, while if in a new message, then it should
        // be treated as an overwrite. For now this is not supported an we will just read the message. If there is two
        // EPCs with the same value in the message, then the last one will win.
        EPC_INSTANCE_LIST_NOTIFICATION => eoj_array_property( // Create. custom type for this as the logic is complex. Also, a custom type can store larger than the amount, as we store decoded.
            "Instance List Notification",
            epc,
            EPC_DOCSOURCE_DEVICE_NONE | EPC_DOCSOURCE_PROFILE_CLASS,
            true,
            EpcAccessRule::NotSupported,
            EpcAccessRule::NotSupported,
        ),
        EPC_SELFNODE_INSTANCE_LIST_S => eoj_array_property( // Create. custom type for this as the logic is complex
            "Self-Node Instance List S",
            epc,
            EPC_DOCSOURCE_DEVICE_NONE | EPC_DOCSOURCE_PROFILE_CLASS,
            false,
            EpcAccessRule::Mandatory,
            EpcAccessRule::NotSupported,
        ),
        EPC_SELFNODE_CLASS_LIST_S => eoj_array_property( // Create. custom type for this as the logic is complex
            "Self-Node Class List S",
            epc,
            EPC_DOCSOURCE_DEVICE_NONE | EPC_DOCSOURCE_PROFILE_CLASS,
            false,
            EpcAccessRule::Mandatory,
            EpcAccessRule::NotSupported,
        ),
        _ => Err(EpcError::NotAvailable(epc))
    }
}
