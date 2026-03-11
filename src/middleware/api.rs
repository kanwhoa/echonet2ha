//! ECHONET related constants
//! Values that are used across wire and middleware implementations.

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

/// ECHONET Lite Object Specification (in-node addressing)
/// A node can contain multiple objects which are addressable through the "ECHONET Lite Object Spefification" (EOJ)
/// * Device Objects. These contain state and properties as per "APPENDIX Detailed Requirements for ECHONET Device objects"
/// * Profile Objects. These define the device capabiltiies and pointers into the device objects
#[derive(Clone, Copy, Debug)]
pub struct EOJ {
    class_group_code: u8, // E.g. sensors, home equipment, etc
    class_code: u8, // The specific type, e.g. a presence sensor
    instance_code: u8 // The instance number of the presence sensor, for example devices that have both PIR and mmWave 
}

/// Implementation methods for EOJ
impl EOJ {
    pub fn from_groupclass_instance(group_class: &GroupClass, instance: u8) -> Self {
        Self {
            class_group_code: group_class.class_group_code,
            class_code: group_class.class_code,
            instance_code: instance,
        }
    }
}

impl std::fmt::Display for EOJ {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:02x}{:02x}{:02x}", self.class_group_code, self.class_code, self.instance_code)
    }
}

/// Holder the group and class (first two bytees of the EOJ)
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GroupClass {
    pub class_group_code: u8, // E.g. sensors, home equipment, etc
    pub class_code: u8, // The specific type, e.g. a presence sensor
}

// Constants for different group/classes
/// Control class
pub const CLASS_CONTROL_CONTROLLER: GroupClass = GroupClass {class_group_code: EOJ_CLASS_GROUP_CONTROL, class_code: 0xff};
/// Profile class
pub const CLASS_PROFILE_NODE_PROFILE: GroupClass = GroupClass {class_group_code: EOJ_CLASS_GROUP_PROFILE, class_code: 0xf0};

impl GroupClass {
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

impl std::fmt::Display for GroupClass {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let (group_desc, class_desc) = self.to_display();
        write!(f, "Group '{}' Class '{}'", group_desc, class_desc)
    }
}

impl From<&EOJ> for GroupClass {
    fn from(value: &EOJ) -> Self {
        Self {
            class_group_code: value.class_group_code,
            class_code: value.class_code
        }
    }
}

/// EPC errors
#[derive(Debug)]
pub enum EpcError {
    /// The EPC code is not valid for this property
    InvalidCode(u8, u8),
    /// The value for the EPC code is not correct (type and/or size)
    InvalidValue(u8),
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
    /// The value is too large
    ValueTooLarge(u8),
    /// Validation of the value failed.
    ValidationFailed(u8),
    /// When using try_into
    TypeConverstionError(String),
}

impl std::fmt::Display for EpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            EpcError::InvalidCode(epc, actual_epc) => write!(f, "EPC({:02x}): Actual code: {:02x}", epc, actual_epc),
            EpcError::InvalidValue(epc) => write!(f, "EPC({:02x}): Invalid value", epc),
            EpcError::InvalidType(epc) => write!(f, "EPC({:02x}): Mismatched canonical type", epc),
            EpcError::NotAvailable(epc) => write!(f, "EPC({:02x}): EPC is not available on this device", epc),
            EpcError::NotSupported(epc) => write!(f, "EPC({:02x}): EPC is not supported on this object class", epc),
            EpcError::OperationNotAllowed(epc) => write!(f, "EPC({:02x}): Operation not allowed by access rule", epc),
            EpcError::OperationNotImplemented(epc) => write!(f, "EPC({:02x}): Operation not implemented by node", epc),
            EpcError::NoValue(epc) => write!(f, "EPC({:02x}): Value is not set", epc),
            EpcError::ValueTooLarge(epc) => write!(f, "EPC({:02x}): Value is larger than the container maximum", epc),
            EpcError::ValidationFailed(epc) => write!(f, "EPC({:02x}): Validation for internal representation failed", epc),
            EpcError::TypeConverstionError(_) => write!(f, "EPC(??): Failed to convert internal type"),
        }
    }
}

impl std::error::Error for EpcError {}

impl From<std::array::TryFromSliceError> for EpcError {
    fn from(value: std::array::TryFromSliceError) -> Self {
        EpcError::TypeConverstionError(value.to_string())
    }
}

impl From<std::num::ParseIntError> for EpcError {
    fn from(value: std::num::ParseIntError) -> Self {
        EpcError::TypeConverstionError(value.to_string())
    }
}

#[derive(Debug)]
pub enum MiddlewareError {
    /// A communications error on a channel or socket
    QueueFailure(String),
}

impl std::fmt::Display for MiddlewareError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            MiddlewareError::QueueFailure(msg) => write!(f, "Queue message failure: {}", msg),
        }
    }
}

impl std::error::Error for MiddlewareError {}

impl<T> From<tokio::sync::mpsc::error::SendError<T>> for MiddlewareError {
    fn from(value: tokio::sync::mpsc::error::SendError<T>) -> Self {
        MiddlewareError::QueueFailure(format!("Failed to send '{}' message to queue as channel was closed", value.to_string()))
    }
}

