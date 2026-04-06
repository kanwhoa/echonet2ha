//! Module containing all of the usable EPC types.
use super::api::{HexString};
use super::api::{Epc, EpcAccessRule, EpcWrapper, EpcError, EOJ};
use super::api::{NodeDeviceObjectEchonetLiteSupportedVersion, NodeObjectFaultContent, NodeObjectUniqueIdentifier, NodeProfileObjectEchonetLiteSupportedVersion, NodeGroupClass, NodeObjectPropertyMap};
use chrono::Datelike;
use std::any::Any;
use std::cell::{Cell, Ref, RefCell};
use std::fmt::{Debug, Display};
use std::ops::Deref;

#[cfg(test)]
mod tests;

///////////////////////////////////////////////////////////////////////////////
// EPC Constants
///////////////////////////////////////////////////////////////////////////////
pub const EPC_OPERATING_STATUS: u8 = 0x80;
pub const EPC_VERSION_INFORMATION: u8 = 0x82;
pub const EPC_IDENTIFICATION_NUMBER: u8 = 0x83;
pub const EPC_FAULT_STATUS: u8 = 0x88;
pub const EPC_FAULT_CONTENT: u8 = 0x89;
pub const EPC_MANUFACTURER_CODE: u8 = 0x8a;
pub const EPC_BUSINESS_FACILITY_CODE: u8 = 0x8b;
pub const EPC_PRODUCT_CODE: u8 = 0x8c;
pub const EPC_SERIAL_NUMBER: u8 = 0x8d;
pub const EPC_PRODUCTION_DATE: u8 = 0x8e;
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
const EPC_FAULT_ENCOUNTERED: u8 = 0x41;
const EPC_FAULT_NOT_ENCOUNTERED: u8 = 0x42;

// What message types are supported
const NODE_MESSAGE_FORMAT_SPECIFIED: u8 = 0x01;
const NODE_MESSAGE_FORMAT_ARBITRARY: u8 = 0x02;

// This is dumb rust, these are absolutely known at compile time...
macro_rules! ERR_MSG_INVALID_BOOLEAN {() => ("did not match true or false value");}
macro_rules! ERR_INVALID_LENGTH {() => ("Invalid length, expected {} bytes, found {}");}

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
    Ok(
        Box::new(
            StaticNodeProperty::new(
                name,
                epc,
                source,
                announce,
                get_operation,
                set_operation,
                Box::new(move |_, canonical| {
                    let mut internal = vec![0x00; 3];
                    internal[2] = if *canonical {true_value} else {false_value};
                    Ok(internal)
                }),
                Box::new(move |epc: &dyn Epc<Canonical = bool>, internal|
                    if internal[2] == true_value {
                        Ok(true)
                    } else if internal[2] == false_value {
                        Ok(false)
                    } else {
                        Err(EpcError::InvalidValue(epc.epc(), ERR_MSG_INVALID_BOOLEAN!().to_owned()))
                    }
                ),
                Box::new(move |epc: &dyn Epc<Canonical = bool>, internal|
                    if internal.len() != 3 {
                        Err(EpcError::InvalidValue(epc.epc(), format!(ERR_INVALID_LENGTH!(), 3, internal.len())))
                    } else if internal[0] == true_value || internal[0] == false_value {
                        return Ok(true)
                    } else {
                        Err(EpcError::InvalidValue(epc.epc(), ERR_MSG_INVALID_BOOLEAN!().to_owned()))
                    }                
                ),
            )
        )
    )
}

/// General handler for properties that should be treated as hex strings.
fn hex_string_property(name: &'static str, epc: u8, source: u16,
    announce: bool, get_operation: EpcAccessRule, set_operation: EpcAccessRule,
    validator: impl Fn(&dyn Epc<Canonical = HexString>, &[u8]) -> Result<bool, EpcError> + 'static) -> Result<Box<dyn EpcWrapper>, EpcError>
{
    Ok(
        Box::new(
            StaticNodeProperty::new(
                name,
                epc,
                source,
                announce,
                get_operation,
                set_operation,
                Box::new(move |epc: &dyn Epc<Canonical = HexString>, canonical| {
                    let mut internal = vec![0x00_u8; canonical.byte_len() + 2];
                    match canonical.decode_into_slice(&mut internal[2..]) {
                        Ok(_) => Ok(internal),
                        Err(err) => Err(EpcError::InvalidValue(epc.epc(), err.to_string()))
                    }
                }),
                Box::new(move |_: &dyn Epc<Canonical = HexString>, internal|
                    Ok((&internal[2..]).into())
                ),
                Box::new(validator),
            )
        )
    )
}

/// General handler for properties that should be treated as ascii strings.
fn ascii_string_property(name: &'static str, epc: u8, source: u16,
    announce: bool, get_operation: EpcAccessRule, set_operation: EpcAccessRule,
    validator: impl Fn(&dyn Epc<Canonical = String>, &[u8]) -> Result<bool, EpcError> + 'static) -> Result<Box<dyn EpcWrapper>, EpcError>
{
    Ok(
        Box::new(
            StaticNodeProperty::new(
                name,
                epc,
                source,
                announce,
                get_operation,
                set_operation,
                Box::new(move |epc: &dyn Epc<Canonical = String>, canonical|
                    if canonical.is_ascii() {
                        let mut internal = vec![0x00_u8; canonical.len() + 2];
                        (&mut internal[2..]).copy_from_slice(canonical.as_bytes());
                        Ok(internal)
                    } else {
                        Err(EpcError::InvalidValue(epc.epc(), "Invalid ASCII characters found".to_owned()))
                    }
                ),
                Box::new(move |_: &dyn Epc<Canonical = String>, internal|
                    unsafe { Ok(String::from_utf8_unchecked((&internal[2..]).to_vec())) }),
                Box::new(move |epc, internal|
                    if (&internal[2..]).iter().all(|&b| (b as char).is_ascii()) {
                        validator(epc, internal)
                    } else {
                        Err(EpcError::InvalidValue(epc.epc(), "Invalid ASCII characters found".to_owned()))
                    }
                )
            )
        )
    )
}

/// Integer trait for constrianing the integer property type
trait Integer {
    const SIZE: usize;
    fn to_be_bytes(self) -> [u8; Self::SIZE];
    fn from_be_bytes(bytes: [u8; Self::SIZE]) -> Self;
}

/// Allow generic u16 integers
impl Integer for u16 {
    const SIZE: usize = std::mem::size_of::<Self>();

    fn to_be_bytes(self) -> [u8; Self::SIZE] {
        self.to_be_bytes()
    }
    
    fn from_be_bytes(bytes: [u8; Self::SIZE]) -> Self {
        u16::from_be_bytes(bytes)
    }
}

/// Allow NodeObjectFaultContent
impl Integer for NodeObjectFaultContent {
    const SIZE: usize = std::mem::size_of::<Self>();

    fn to_be_bytes(self) -> [u8; Self::SIZE] {
        self.0.to_be_bytes()
    }
    
    fn from_be_bytes(bytes: [u8; Self::SIZE]) -> Self {
        NodeObjectFaultContent(u16::from_be_bytes(bytes))
    }
}

/// Allow NodeObjectUniqueIdentifier
impl Integer for NodeObjectUniqueIdentifier {
    const SIZE: usize = std::mem::size_of::<Self>();

    fn to_be_bytes(self) -> [u8; Self::SIZE] {
        self.0.to_be_bytes()
    }
    
    fn from_be_bytes(bytes: [u8; Self::SIZE]) -> Self {
        NodeObjectUniqueIdentifier(u16::from_be_bytes(bytes))
    }
}

/// Generic handler for integer type properties
fn integer_property<T>(name: &'static str, epc: u8, source: u16,
    announce: bool, get_operation: EpcAccessRule, set_operation: EpcAccessRule,
    validator: impl Fn(&dyn Epc<Canonical = T>, &[u8]) -> Result<bool, EpcError> + 'static) -> Result<Box<dyn EpcWrapper>, EpcError>
where
    T: Integer + Display + Debug + Copy + 'static,
    [(); T::SIZE]: Sized
{
    Ok(
        Box::new(
            StaticNodeProperty::new(
                name,
                epc,
                source,
                announce,
                get_operation,
                set_operation,
                Box::new(move |_: &dyn Epc<Canonical = T>, canonical| {
                    let mut internal = vec![0x00_u8; T::SIZE + 2];
                    (&mut internal[2..]).copy_from_slice(&(canonical.to_be_bytes()));
                    Ok(internal)
                }),
                Box::new(move |_: &dyn Epc<Canonical = T>, internal| {
                    let buf: [u8; T::SIZE] = (&internal[2..]).try_into().unwrap();
                    Ok(T::from_be_bytes(buf))
                }),
                Box::new(move |epc, internal|
                    if internal.len() == T::SIZE + 2 {
                        validator(epc, internal)
                    } else {
                        Err(EpcError::InvalidValue(epc.epc(), format!(ERR_INVALID_LENGTH!(), T::SIZE + 2, internal.len())))
                    }
                ),
            )
        )
    )
}

/// General handler for date types
fn date_property(name: &'static str, epc: u8, source: u16,
    announce: bool, get_operation: EpcAccessRule, set_operation: EpcAccessRule,
    validator: impl Fn(&dyn Epc<Canonical = chrono::NaiveDate>, &[u8]) -> Result<bool, EpcError> + 'static) -> Result<Box<dyn EpcWrapper>, EpcError>
{
    Ok(
        Box::new(
            StaticNodeProperty::new(
                name,
                epc,
                source,
                announce,
                get_operation,
                set_operation,
                Box::new(move |_: &dyn Epc<Canonical = chrono::NaiveDate>, canonical| {
                    let mut buf = vec![0x00; 6];
                    (&mut buf[2..4]).copy_from_slice(&(canonical.year() as i16).to_be_bytes());
                    buf[4] = canonical.month() as u8;
                    buf[5] = canonical.day() as u8;
                    Ok(buf)
                }),
                Box::new(move |epc: &dyn Epc<Canonical = chrono::NaiveDate>, internal| {
                    let year = u16::from_be_bytes(internal[2..4].try_into().unwrap()) as i32;
                    let month = internal[4] as u32;
                    let day = internal[5] as u32;

                    let maybe_canonical = chrono::NaiveDate::from_ymd_opt(year, month, day);
                    if let Some(canonical) = maybe_canonical {
                        Ok(canonical)
                    } else {
                        return Err(EpcError::InvalidValue(epc.epc(), "error parsing date".to_owned()));
                    }
                }),
                Box::new(move |epc, internal|
                    if internal.len() == 6 {
                        validator(epc, internal)
                    } else {
                        Err(EpcError::InvalidValue(epc.epc(), format!(ERR_INVALID_LENGTH!(), 6, internal.len())))
                    }
                )
            )
        )
    )
}

/// Handler for NodePropertyMap types
fn property_map_property(name: &'static str, epc: u8, source: u16,
    announce: bool, get_operation: EpcAccessRule, set_operation: EpcAccessRule) -> Result<Box<dyn EpcWrapper>, EpcError>
{    
    Ok(
        Box::new(
            StaticNodeProperty::new(
                name,
                epc,
                source,
                announce,
                get_operation,
                set_operation,
                Box::new(move |_: &dyn Epc<Canonical = NodeObjectPropertyMap>, canonical| {
                    // To avoid a copy we need to calculate the size of the buffer needed based on the type. This required
                    // knowledge of how the struct will serialise.
                    // FIXME 
                    Ok(canonical.decode())
                }),
                Box::new(move |epc: &dyn Epc<Canonical = NodeObjectPropertyMap>, internal| 
                    match NodeObjectPropertyMap::from_bytes(&internal[2..]) {
                        Ok(canonical) => Ok(canonical),
                        Err(err) => Err(EpcError::InvalidValue(epc.epc(), format!("{}", err)))
                    }
                ),
                Box::new(move |epc, internal|
                    if internal.len() > 2 && ((internal[1] < 16 && ((internal[1] + 2) as usize == internal.len())) || internal.len() == 17) {
                        Ok(true)
                    } else {
                        Err(EpcError::InvalidValue(epc.epc(), "Invalid data length".to_owned()))
                    }
                )
            )
        )
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
    Ok(
        Box::new(
            StaticNodeProperty::new(
                name,
                epc,
                source,
                announce,
                get_operation,
                set_operation,
                Box::new(move |_: &dyn Epc<Canonical = EOJVec>, canonical|
                    todo!()
                ),
                Box::new(move |epc: &dyn Epc<Canonical = EOJVec>, internal| 
                    todo!()
                ),
                Box::new(move |epc, internal|
                    // Simple validator. May decide we need to verify the EPC codes at a later time.
                    Ok(internal.len() >= 2 && (internal[1] as usize) % 3 == 0) 
                )
            )
        )
    )
}

/// A simple length validator support function
fn validate_length(epc_code: u8, internal: &[u8], len: usize) -> Result<bool, EpcError> {
    if internal.len() == len {
        Ok(true)
    } else {
        Err(EpcError::InvalidValue(epc_code, format!("expected {} bytes found {}", len, internal.len())))
    }
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
            // This type is special. It contains the version supported AND the message type supported. However,
            // in ECHONET Lite, only the "specified message format" is supported, meaning that the if the device
            // advertisies "arbitrary message format", chances are we won't be able to interpret any messages from
            // the device. The actual message format is stored in EHD1/EHD2 headers. For here, we will just store
            // the value. This is complex because the device and profile types/uses differ significantly.
            EPC_VERSION_INFORMATION => Ok(if is_profile {
                Box::new(StaticNodeProperty::new(
                    "Version Information",
                    epc,
                    EPC_DOCSOURCE_DEVICE_SUPERCLASS | EPC_DOCSOURCE_PROFILE_CLASS,
                    false,
                    EpcAccessRule::Mandatory,
                    EpcAccessRule::NotSupported,
                    Box::new(move |np: &dyn Epc<Canonical = NodeProfileObjectEchonetLiteSupportedVersion>, canonical|
                        if canonical.specified_message || canonical.arbiturary_message {
                            let mut internal = vec![0x00_u8; 6];
                            internal[2] = canonical.major_version;
                            internal[3] = canonical.minor_version;
                            internal[4] = if canonical.specified_message {NODE_MESSAGE_FORMAT_SPECIFIED} else {0x00} | if canonical.arbiturary_message {NODE_MESSAGE_FORMAT_ARBITRARY} else {0x00};
                            Ok(internal)
                        } else {
                            return Err(EpcError::InvalidValue(np.epc(), "specified or arbitrary messages must be supported".to_owned()))
                        }
                    ),
                    Box::new(move |_, internal| Ok(NodeProfileObjectEchonetLiteSupportedVersion {
                        major_version: internal[2],
                        minor_version: internal[3],
                        specified_message: (internal[4] & NODE_MESSAGE_FORMAT_SPECIFIED) == NODE_MESSAGE_FORMAT_SPECIFIED,
                        arbiturary_message: (internal[4] & NODE_MESSAGE_FORMAT_ARBITRARY) == NODE_MESSAGE_FORMAT_ARBITRARY,
                    })),
                    Box::new(move |epc, internal|
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
                )) as Box<dyn EpcWrapper>
            } else {
                Box::new(StaticNodeProperty::new(
                    "Version Information",
                    epc,
                    EPC_DOCSOURCE_DEVICE_SUPERCLASS | EPC_DOCSOURCE_PROFILE_CLASS,
                    false,
                    EpcAccessRule::Mandatory,
                    EpcAccessRule::NotSupported,
                    Box::new(move |_: &dyn Epc<Canonical = NodeDeviceObjectEchonetLiteSupportedVersion>, canonical| {
                        let mut internal = vec![0x00_u8; 6];
                        internal[4] = canonical.release.to_ascii_uppercase() as u8;
                        internal[5] = canonical.revision;
                        Ok(internal)
                    }),
                    Box::new(move |_, internal| Ok(NodeDeviceObjectEchonetLiteSupportedVersion {
                        release: internal[2] as char,
                        revision: internal[3],
                    })),
                    Box::new(move |epc, internal|
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
                )) as Box<dyn EpcWrapper>
            }),
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
            // Fault status. True if a fault is encountered
            EPC_FAULT_STATUS => boolean_property(
                "Fault Status",
                epc,
                EPC_DOCSOURCE_DEVICE_SUPERCLASS | EPC_DOCSOURCE_PROFILE_SUPERCLASS,
                if is_profile {false} else {true},
                EpcAccessRule::Supported,
                EpcAccessRule::NotSupported,
                EPC_FAULT_ENCOUNTERED,
                EPC_FAULT_NOT_ENCOUNTERED
            ),
            // Fault content: 0x0000 to 0x03E8 same as device. 0x03E9 to 0x03EC: abnormality codes of ECHONET Lite middleware
            // adapters described in "Part III, ECHONET Lite Communications Equipment Specifications."
            // FIXME: update this to cover the device type. Change from u16 to a custom class.
            EPC_FAULT_CONTENT => integer_property::<NodeObjectFaultContent>(
                "Fault Content (Fault Description)",
                epc,
                EPC_DOCSOURCE_DEVICE_SUPERCLASS | EPC_DOCSOURCE_PROFILE_CLASS,
                false,
                EpcAccessRule::Supported,
                EpcAccessRule::NotSupported,
                move |epc, _| {
                    let val = epc.to_canonical()?;
                    if 0x0000 < (*val) && (*val) <= 0x03ec {
                        Ok(true)
                    } else {
                        Err(EpcError::InvalidValue(epc.epc(), "Value not in range 0x0000 - 0x03ec".to_owned()))
                    }
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
            EPC_UNIQUE_IDENTIFIER_DATA => integer_property::<NodeObjectUniqueIdentifier>(
                "Unique Identifier Data",
                epc,
                EPC_DOCSOURCE_DEVICE_NONE | EPC_DOCSOURCE_PROFILE_CLASS,
                false,
                EpcAccessRule::Supported,
                EpcAccessRule::Supported,
                move |_, _| Ok(true)
            ),
            EPC_NUMBER_OF_SELFNODE_INSTANCES => integer_property::<u16>( // FIXME: this is a three byte integer!!!
                "Number of Self-Node Instances",
                epc,
                EPC_DOCSOURCE_DEVICE_NONE | EPC_DOCSOURCE_PROFILE_CLASS,
                false,
                EpcAccessRule::Mandatory,
                EpcAccessRule::NotSupported,
                move |_, _| Ok(true)
            ),
            EPC_NUMBER_OF_SELFNODE_CLASSES => integer_property::<u16>(
                "Number of Self-Node Classes",
                epc,
                EPC_DOCSOURCE_DEVICE_NONE | EPC_DOCSOURCE_PROFILE_CLASS,
                false,
                EpcAccessRule::Mandatory,
                EpcAccessRule::NotSupported,
                move |_, _| Ok(true)
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
    } else if epc >= 0xa0 && epc < 0xf0 {
        // Should be group specific, but maybe not. Try group first.
        // Create a function to check the group, if not, try the class.
        // group epc >= 0xa0 && epc < 0xc0
        // class epc >= 0xc0 && epc < 0xf0
        Err(EpcError::NotAvailable(epc))
    } else {
        // User defined area.
        Err(EpcError::NotAvailable(epc))
    }?;

    // Configure the default
    Ok(property)
}

