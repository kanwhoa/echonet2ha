//! ECHONET Middleware implementation
//! The middleware only deals with events.
pub mod events;
pub mod api;

// Manufacturer codes
const MANUFACTURER_UNREGISTERED: [u8; 3] = [0xff, 0xff, 0xff];

// Node types
const NODE_TYPE_GENERAL: u8 = 0x01;
const NODE_TYPE_TRANSMIT_ONLY: u8 = 0x01;

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
enum NodeType {
    General,
    TransmitOnly
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

/// Traits to represent the canonical data format. Vec<u8> is the generic form.
trait NodePropertyCanonicalType {}
impl NodePropertyCanonicalType for Vec<u8> {}
impl NodePropertyCanonicalType for bool {}
trait NodePropertyGenericType: std::any::Any {
    fn get_epc(&self) -> u8;
    fn as_any(&self) -> &dyn std::any::Any;
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}

// The type aliases the to_canonical and from_canonical functions. This is because multiple traits are not allowed
// on the definition, but it also works to keep consistency. Closing the closure is very difficult and requires
// a bunch of hoops. As such, we'll just use a Reference Count (Rc) to store against the original.
type FromCanonicalType<T: NodePropertyCanonicalType> = fn(&NodeProperty<T>, &T) -> Result<Vec<u8>, api::EpcError>;
type ToCanonicalType<T: NodePropertyCanonicalType> = fn(&NodeProperty<T>, &Vec<u8>) -> Result<T, api::EpcError>;

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
    to_canonical: ToCanonicalType<T>
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
            to_canonical: self.to_canonical.clone()
        }
    }
}

// General node property implementation
impl<T: NodePropertyCanonicalType> NodeProperty<T> {
    /// Constructor
    const fn new(name: &'static str, epc: u8, announce: bool, get_operation: NodePropertyOperation, set_operation: NodePropertyOperation, from_canonical: FromCanonicalType<T>, to_canonical: ToCanonicalType<T>) -> NodeProperty<T>
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
        }
    }

    /// Clone an existing property and set the default value from a canonical value
    fn clone_with_canonical_value(&self, value: &T) -> Result<NodeProperty<T>, api::EpcError> {
        let cloned = self.clone();

        // Set the data using the from_canonical function
        let mut cell_value = self.last_value.borrow_mut();
        *cell_value = (self.from_canonical)(self, value)?;
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
        // Check the value exists
        if self.last_updated.get() == 0 {
            return Err(api::EpcError::NoValue(self.epc));
        }

        // Get the current value. Convert to canonical to validate before sending.
        let cell_value = &*self.last_value.borrow();
        (self.to_canonical)(self, cell_value)?;

        if cell_value.len() > 0xff {
            return Err(api::EpcError::ValueTooLarge(self.epc));
        }

        // Package into a wire value
        let mut wire: Vec<u8> = Vec::with_capacity(cell_value.len() + 2);
        wire[0] = self.epc;
        wire[1] = wire.len() as u8;
        let epc_data = &mut wire[2..][..cell_value.len()];
        wire.copy_from_slice(cell_value);

        Ok(wire)
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
        } else {
            // Try and convert to the canonical form, which will invoke the validators.
            let epc_data = epc_buf[2..].to_vec();
            (self.to_canonical)(self, &epc_data)?;

            // All good, now set the value
            let mut cell_value = self.last_value.borrow_mut();
            *cell_value = epc_data;
            self.last_updated.set(chrono::Utc::now().timestamp_millis());
        }

        Ok(())
    }

    /// Get and return an owned canonical version of the internal struct
    fn get_canonical(&self) -> Result<T, api::EpcError> {
        // Check the value exists
        if self.last_updated.get() == 0 {
            return Err(api::EpcError::NoValue(self.epc));
        }

        let cell_value = &*self.last_value.borrow();
        (self.to_canonical)(self, cell_value)
    }

    /// Set the internal state from a canonical version of the data
    fn set_canonical(&self, canonical: &T) -> Result<(), api::EpcError> {
        let epc_data = (self.from_canonical)(self, canonical)?;
        if epc_data.len() > 0xff {
            return Err(api::EpcError::ValueTooLarge(self.epc));
        }

        let mut cell_value = self.last_value.borrow_mut();
        *cell_value = epc_data;
        self.last_updated.set(chrono::Utc::now().timestamp_millis());
        Ok(())
    }

    /// Return a borrowed reference to the underlying vec. No checks on the data are
    /// performed. The caller is expected to know the type and size.
    fn get_internal(&self) -> Result<std::cell::Ref<'_, Vec<u8>>, api::EpcError> {
        if self.last_updated.get() == 0 {
            return Err(api::EpcError::NoValue(self.epc));
        }
        Ok(self.last_value.borrow())
    }
}

/// From a Vec<u8> canonical type.
#[inline(always)]
fn from_vec(np: &NodeProperty<Vec<u8>>, canonical: &Vec<u8>, validate: fn(&Vec<u8>) -> bool) -> Result<Vec<u8>, api::EpcError> {
    if !validate(canonical) {
        return Err(api::EpcError::InvalidValue(np.epc));
    }
    Ok(canonical.clone())
}
/// To a bool<u8> canonical type
#[inline(always)]
fn to_vec(np: &NodeProperty<Vec<u8>>, internal: &Vec<u8>, validate: fn(&Vec<u8>) -> bool) -> Result<Vec<u8>, api::EpcError> {
    if !validate(internal) {
        return Err(api::EpcError::InvalidValue(np.epc));
    }
    Ok(internal.clone())
}

/// From a bool canonical type
#[inline(always)]
fn from_bool(_np: &NodeProperty<bool>, canonical: &bool, true_value: u8, false_value: u8) -> Result<Vec<u8>, api::EpcError> {
    Ok([if *canonical { true_value } else { false_value }].to_vec())
}
/// To a bool canonical type
#[inline(always)]
fn to_bool(np: &NodeProperty<bool>, internal: &Vec<u8>, true_value: u8, false_value: u8) -> Result<bool, api::EpcError> {
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
    |np, internal| to_bool(np, internal, EPC_OPERATION_STATUS_ON, EPC_OPERATION_STATUS_OFF)
);
/// This type is special. It contains the version supported AND the message type supported. However,
/// in ECHONET Lite, only the "specified message format" is supported, meaning that the if the device
/// advertisies "arbitrary message format", chances are we won't be able to interpret any messages from
/// the device. The actual message format is stored in EHD1/EHD2 headers. For here, we will just store
/// the value.
const NODE_PROPERTY_VERSION_INFORMATION: NodeProperty<Vec<u8>> = NodeProperty::new(
    "Version Information",
    EPC_VERSION_INFORMATION,
    false,
    NodePropertyOperation::Mandatory,
    NodePropertyOperation::NotSupported,
    |np, canonical| from_vec(np, canonical, |buf| buf.len() == 4),
    |np, internal| to_vec(np, internal, |buf| buf.len() == 4),
);
/// This stores two values. The whole value can be considered the identification number, with the
/// first byte identifying the communication medium. In ECHONET Lite, this is fixed at 0xfe. This also means
/// that the rest of the data is the "manufacturer specified format". This is basically 3 bytes identifying
/// the manufacturer and then the remaining 13 bytes specified by the manufacturer.
const NODE_PROPERTY_IDENTIFICATION_NUMBER: NodeProperty<Vec<u8>> = NodeProperty::new(
    "Version Information",
    EPC_IDENTIFICATION_NUMBER,
    false,
    NodePropertyOperation::Mandatory,
    NodePropertyOperation::NotSupported,
    |np, canonical| from_vec(np, canonical, |buf| buf.len() == 17 && buf[0] == 0xfe),
    |np, internal| to_vec(np, internal, |buf| buf.len() == 17 && buf[0] == 0xfe),
);
/// The fault status. true == fault. False ==  no fault.
const NODE_PROPERTY_FAULT_STATUS: NodeProperty<bool> = NodeProperty::new(
    "Fault Status",
    EPC_FAULT_STATUS,
    true,
    NodePropertyOperation::Supported,
    NodePropertyOperation::NotSupported,
    |np, canonical| from_bool(np, canonical, EPC_FAULT_ENCOUNTERED, EPC_FAULT_NOT_ENCOUNTERED),    
    |np, internal| to_bool(np, internal, EPC_FAULT_ENCOUNTERED, EPC_FAULT_NOT_ENCOUNTERED)
);
/// Manufacturer code. See also [NODE_PROPERTY_IDENTIFICATION_NUMBER]
const NODE_PROPERTY_MANUFACTURER_CODE: NodeProperty<Vec<u8>> = NodeProperty::new(
    "Manufacturer code",
    EPC_MANUFACTURER_CODE,
    false,
    NodePropertyOperation::Mandatory,
    NodePropertyOperation::NotSupported,
    |np, canonical| from_vec(np, canonical, |buf| buf.len() == 3),
    |np, internal| to_vec(np, internal, |buf| buf.len() == 3),
);

/// ECHONET Lite Object Specification (in-node addressing)
/// A node can contain multiple objects which are addressable through the "ECHONET Lite Object Spefification" (EOJ)
/// * Device Objects. These contain state and properties as per "APPENDIX Detailed Requirements for ECHONET Device objects"
/// * Profile Objects. These
struct EOJ {
    class_group_code: u8, // E.g. sensors, home equipment, etc
    class_code: u8, // The specific type, e.g. a presence sensor
    instance_code: u8 // The instance number of the presence sensor, for example devices that have both PIR and mmWave 
}

/// ECHONET Object representation (both device and profile)
struct NodeObject {
    eoj: EOJ,
    properties: Vec<Box<dyn NodePropertyGenericType>>
}

impl NodeObject {
    /// Create a device object
    fn device(class_group_code: u8, class_code: u8, instance: u8) -> Self {
        // All objects has a standard set of superclass objects
        // APPENDIX Detailed Requirements for ECHONET Device objects: Device Object Super Class Requirements

        NodeObject {
            eoj: EOJ {class_group_code: class_group_code, class_code: class_code, instance_code: instance},
            properties: vec![
            ]
        }
    }

    /// Create a profile object
    fn profile(r#type: NodeType, manufacturer_code: &[u8; 3], instance: u64) -> Self {
        // All objects has a standard set of superclass objects
        // Communication Middleware Specifications: Profile Object Class Group Specifications

        // Create the unique identification number for this node.
        let instance_bytes = instance.to_be_bytes();
        let identification_number_size = 17;
        let mut identification_number = Vec::with_capacity(identification_number_size);
        identification_number.resize(identification_number_size, 0x00);
        identification_number[0] = 0xfe;
        identification_number[1..4].copy_from_slice(manufacturer_code);
        identification_number[(identification_number_size - 1 - std::mem::size_of::<u64>())..(identification_number_size-1)].copy_from_slice(&instance_bytes);

        // Construct the node object
        NodeObject {
            eoj: EOJ {
                class_group_code: api::CLASS_PROFILE_NODE_PROFILE.class_group_code,
                class_code: api::CLASS_PROFILE_NODE_PROFILE.class_code,
                instance_code: match r#type {
                    NodeType::General => NODE_TYPE_GENERAL,
                    NodeType::TransmitOnly => NODE_TYPE_TRANSMIT_ONLY
                }
            },
            properties: vec![
                // Superclass specifications (6.10.1 Overview of Profile Object Super Class Specifications)
                Box::new(NODE_PROPERTY_FAULT_STATUS.clone_with_canonical_value(&false).unwrap()),
                Box::new(NODE_PROPERTY_MANUFACTURER_CODE.clone_with_canonical_value(&manufacturer_code.to_vec()).unwrap()),

                // Node profile class (6.11.1 Node Profile Class: Detailed Specifications)
                Box::new(NODE_PROPERTY_OPERATING_STATUS.clone_with_canonical_value(&true).unwrap()),
                Box::new(NODE_PROPERTY_VERSION_INFORMATION.clone_with_canonical_value(&[api::ECHONET_MAJOR_VERSION, api::ECHONET_MINOR_VERSION, 0x01, 0x00].to_vec()).unwrap()),
                Box::new(NODE_PROPERTY_IDENTIFICATION_NUMBER.clone_with_canonical_value(&identification_number).unwrap()),
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
        let manufacturer_code = &MANUFACTURER_UNREGISTERED;

        // Create the standard middleware instance, and default objects.
        let middleware = Self {
            self_node: Node {
                physical_address: NodeAddress::Localhost,
                r#type: NodeType::General,
                profile_object: NodeObject::profile(NodeType::General, manufacturer_code, instance),
                device_objects: vec![
                    NodeObject::device(api::CLASS_CONTROL_CONTROLLER.class_group_code, api::CLASS_CONTROL_CONTROLLER.class_code, 1)
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

        // Set the bridge to online and send an update
        
        // Send an announcement message
        self.broadcast_tx.send(events::Event::Announce).await?;

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
