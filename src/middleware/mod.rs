//! ECHONET Middleware implementation
//! The middleware only deals with events.
pub mod api;
pub mod epc_types;
pub mod events;
//pub mod properties;

//#[cfg(test)]
//mod tests;

use api::{EpcWrapper, NodeType};

// Constants
/// Maximum size of an EPC data block
/// Needs two bytes for the type and length
const MAX_EPC_LEN: usize = 0xfd;

// Node types
const NODE_TYPE_ALL: u8 = 0x00;
const NODE_TYPE_GENERAL: u8 = 0x01;
const NODE_TYPE_TRANSMIT_ONLY: u8 = 0x02;

// What message types are supported
const NODE_MESSAGE_FORMAT_SPECIFIED: u8 = 0x01;
const NODE_MESSAGE_FORMAT_ARBITRARY: u8 = 0x02;

// What capabilities does an property support
const NODE_VALUE_OPERATION_GET_AVAILABLE: u8 = 0x01;
const NODE_VALUE_OPERATION_GET_SUPPORTED: u8 = 0x02;
const NODE_VALUE_OPERATION_GET_MANDATORY: u8 = 0x04;
const NODE_VALUE_OPERATION_SET_AVAILABLE: u8 = 0x10;
const NODE_VALUE_OPERATION_SET_SUPPORTED: u8 = 0x20;
const NODE_VALUE_OPERATION_SET_MANDATORY: u8 = 0x40;

/*

/// Traits to represent the canonical data format. Vec<u8> is the generic form.
trait NodePropertyCanonicalType: Debug + Display {}
// Conversions to specific types
impl NodePropertyCanonicalType for Vec<u8> {}
impl NodePropertyCanonicalType for bool {}
impl NodePropertyCanonicalType for chrono::NaiveDate {}
impl NodePropertyCanonicalType for String {}
impl NodePropertyCanonicalType for NodeEchonetLiteSupportedVersion {}
impl NodePropertyCanonicalType for NodePropertyMap {}



/// Static implementation of an ECHONET property.
struct StaticNodeProperty<T: NodePropertyCanonicalType>
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

// General node property implementation
impl<T: NodePropertyCanonicalType> StaticNodeProperty<T> {
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

/// The EPC operations for StaticNodeProperty
impl<T: NodePropertyCanonicalType> EPC<T> for StaticNodeProperty<T> {
    /// See [EPC::as_any]
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    /// See [EPC::as_any_mut]
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    /// See [EPC::epc]
    fn epc(&self) -> u8 {
        self.epc
    }

    /// See [EPC::name]
    fn name(&self) -> &str {
        self.name
    }
    
    /// See [EPC::accept_wire]
    fn accept_wire(&self, wire: &[u8]) -> bool {
        wire.len() >= 2 && wire[0] == self.epc
    }
    
    fn to_wire(&self) -> Result<Vec<u8>, api::ConversionError> {
        todo!()
    }
    
    fn from_wire(wire: &[u8]) -> Result<Self, api::ConversionError> {
        todo!()
    }
    
    fn get(&self) -> Result<std::cell::Ref<'_, Vec<u8>>, api::EpcError> {
        todo!()
    }
    
    fn set(&self, internal: &[u8]) -> Result<(), api::EpcError> {
        todo!()
    }
    
    fn to_canonical(&self) -> Result<T, api::EpcError> {
        todo!()
    }
    
    fn from_canonical(&self, canonical: &T) -> Result<(), api::EpcError> {
        todo!()
    }
    
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
    */

/// ECHONET Object representation (both device and profile)
struct NodeObject {
    eoj: api::EOJ,
    // FIXME: this is horrible....
    properties: Vec<Box<dyn EpcWrapper>>
}

impl NodeObject {
    /// Create a device object
    fn device(group_class: &api::NodeGroupClass, instance: u8) -> Self {
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

        // Prevent the constant being created lots of times.
        let group_class = &api::CLASS_PROFILE_NODE_PROFILE;

        // TODO: instead of having a canonical form of vec, should it be a string of hex, or in the HA adapter, do we convert from hex string to hex array?
        // Construct the node object
        NodeObject {
            eoj: api::EOJ::from_groupclass_instance(&group_class, r#type as u8),
            properties: vec![
                // Superclass specifications (6.10.1 Overview of Profile Object Super Class Specifications)
                epc_types::property_factory(group_class, epc_types::EPC_OPERATING_STATUS).unwrap()

                //Box::new(properties::NODE_PROPERTY_FAULT_STATUS.clone_with_canonical_value(&false).unwrap()),
                //Box::new(properties::NODE_PROPERTY_MANUFACTURER_CODE.clone_with_internal_value(manufacturer_code).unwrap()),
                /*
                Box::new(properties::NODE_PROPERTY_ANNOUNCEMENT_PROPERTY_MAP.clone_with_dynamic_internal_value(???).unwrap()),
                Box::new(properties::NODE_PROPERTY_SET_PROPERTY_MAP.clone_with_dynamic_value(???).unwrap()),
                Box::new(properties::NODE_PROPERTY_GET_PROPERTY_MAP.clone_with_dynamic_value(???).unwrap()),*/

                // Node profile class (6.11.1 Node Profile Class: Detailed Specifications)
                /*Box::new(properties::NODE_PROPERTY_OPERATING_STATUS.clone_with_canonical_value(&true).unwrap()),
                Box::new(properties::NODE_PROPERTY_VERSION_INFORMATION.clone_with_canonical_value(&NodeEchonetLiteSupportedVersion {
                    major_version: api::ECHONET_MAJOR_VERSION,
                    minor_version: api::ECHONET_MINOR_VERSION,
                    specified_message: true,
                    arbiturary_message: false
                }).unwrap()),
                Box::new(properties::NODE_PROPERTY_IDENTIFICATION_NUMBER.clone_with_internal_value(identification_number.as_slice()).unwrap()),
                // TODO: self node instances, self node classes, node instant list S, node class list S*/
            ]
            // TODO: other properties
        }
    }

    /*
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
    */

}

/// Definition of a node. Nodes can have different traits/classes?
/// Instances may not be in sequential order, for example when there are
/// gaps in the addressing/capabilities.
struct Node {
    /// The pyshical address the device was last seen at. This is not the primary key
    /// since device on the network can change addresses by DHCP (though likely not SLAAC)
    physical_address: api::NodePhysicalAddress,
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
    /*fn unique_identifier(&self) -> Result<[u8; 16], api::EpcError> {
        let property_result = self.profile_object.get_node_property_by_template(&properties::NODE_PROPERTY_IDENTIFICATION_NUMBER);
        if let Ok(actual) = property_result {
            let data = actual.get_internal()?;
            Ok((&data[1..]).try_into()?)
        } else {    
            Err(api::EpcError::NotAvailable(properties::EPC_IDENTIFICATION_NUMBER))
        }
    }*/

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
                physical_address: api::NodePhysicalAddress::Localhost,
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