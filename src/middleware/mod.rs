//! ECHONET Middleware implementation
//! The middleware only deals with events.
pub mod api;
pub mod events;
pub mod properties;

#[cfg(test)]
mod tests;

// Constants
/// Maximum size of an EPC data block
/// Needs two bytes for the type and length
const MAX_EPC_LEN: usize = 0xfd;

// Node types
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

/// Node capabilities
#[derive(PartialEq, Eq, Debug)]
#[repr(u8)]
enum NodeType {
    General = NODE_TYPE_GENERAL,
    TransmitOnly = NODE_TYPE_TRANSMIT_ONLY
}

/// Get/Set access rules. Announce is managed separately.
/// If not supported, then announce must be false.
#[derive(PartialEq, Eq, Debug)]
pub enum NodePropertyOperation {
    /// The Get or Set operation is NOT supported
    NotSupported,
    /// The Get or Set operation is supported
    Supported,
    /// The Get or Set operation is mandatory. Implies supported.
    Mandatory,
}

/// Holder for the version information and message types
pub struct NodeEchonetLiteSupportedVersion {
    pub(in super) major_version: u8,
    pub(in super) minor_version: u8,
    pub specified_message: bool,
    pub arbiturary_message: bool,
}

impl NodeEchonetLiteSupportedVersion {
    pub fn version(&self) -> String {
        format!("{}.{}", self.major_version, self.minor_version)
    }
}

impl api::WirePresentable for NodeEchonetLiteSupportedVersion {
    fn to_wire(&self) -> Result<Vec<u8>, api::ConversionError> {
        if !self.specified_message && !self.arbiturary_message {
            Err(api::ConversionError::SerialisationFailed("No supported message types".to_owned()))
        } else {
            let mut internal = vec![0x00; 4];
            internal[0] = self.major_version;
            internal[1] = self.minor_version;
            internal[2] = if self.specified_message {NODE_MESSAGE_FORMAT_SPECIFIED} else {0x00} | if self.arbiturary_message {NODE_MESSAGE_FORMAT_ARBITRARY} else {0x00};
            internal[3] = 0x00;
            Ok(internal)
        }
    }

    fn from_wire(wire: &[u8]) -> Result<Self, api::ConversionError> {
        if wire.len() != 4 {
            Err(api::ConversionError::DeserialisationFailed("Buffer is incorrectly sized".to_owned()))
        } else {
            Ok(NodeEchonetLiteSupportedVersion {
                major_version: wire[0],
                minor_version: wire[1],
                specified_message: (wire[2] & NODE_MESSAGE_FORMAT_SPECIFIED) == NODE_MESSAGE_FORMAT_SPECIFIED,
                arbiturary_message: (wire[2] & NODE_MESSAGE_FORMAT_ARBITRARY) == NODE_MESSAGE_FORMAT_ARBITRARY,
            })
        }
    }
}

/// Holder for the supported EPC property maps
pub struct NodePropertyMap {
    /// byte 0xn0 + 0x80 operations
    operations: [u16; 8],
    operations_count: usize
}

impl NodePropertyMap {
    fn new() -> Self {
        NodePropertyMap{operations: [0x0000; 8], operations_count: 0}
    }

    /// Set the operation as enabled.
    fn enable_operation(&mut self, operation: u8) -> Result<(), api::MiddlewareError> {
        if !self.operation_enabled(operation)? {
            self.operations[((operation - 0x80) >> 4) as usize] |= 0x0001 << (operation & 0x0f);
            self.operations_count += 1;
        }
        Ok(())
    }

    /// Disable an operation
    fn disable_operation(&mut self, operation: u8) -> Result<(), api::MiddlewareError> {
        if self.operation_enabled(operation)? {
            self.operations[((operation - 0x80) >> 4) as usize] &= !(0x0001 << (operation & 0x0f));
            self.operations_count -= 1;
        }
        Ok(())
    }

    /// Disable all operations
    fn disable_all(&mut self) -> Result<(), api::MiddlewareError> {
        self.operations = [0x0000; 8];
        Ok(())
    }

    /// Determines if an operation is enabled
    /// 
    /// # Arguments
    /// * `operation` - The operation to check (range 0x80 - 0xff inclusive)
    /// 
    /// # Returns
    /// Ok(true) if the operation is enabled, Ok(false) otherwise. Err if the
    /// operation value is invalid.
    fn operation_enabled(&self, operation: u8) -> Result<bool, api::MiddlewareError> {
        self.validate_operation(operation)?;
        Ok(self.operations[((operation - 0x80) >> 4) as usize] & (0x0001 << (operation & 0x0f)) != 0)
    }

    fn validate_operation(&self, operation: u8) -> Result<(), api::MiddlewareError> {
        if operation < 0x80 {
            return Err(api::MiddlewareError::InvalidValue("valid operations must be in the range 0x80 - 0xff inclusive".to_owned()));
        }
        Ok(())
    }
}

impl std::fmt::Debug for NodePropertyMap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut dbg = f.debug_struct("NodePropertyMap");
        
        dbg.field("operations_count", &self.operations_count);
        for i in 0..8 {
            let a = format!("{:02x}: {:08b} {:08b}", (i << 4) + 0x80, (self.operations[i] & 0xff00) >> 8, self.operations[i] & 0xff);
            dbg.field(format!("operation[{:02x}]", i).as_str(), &a);
        }

        dbg.finish()
    }
}

impl api::WirePresentable for NodePropertyMap {
    /// A tradeoff here. To have a more efficient conversion, we need to track the count,
    /// which means the set and clear operations 
    fn to_wire(&self) -> Result<Vec<u8>, api::ConversionError> {
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
            Ok(wire)
        } else {
            let mut wire = vec![0x00; 17];
            wire[0] = self.operations_count as u8;

            // Transpose. Would be quicker and easier with SIMD instructions, but that is not stable yet, and this is small
            for i in 0..8 {
                for j in 0..16 {
                    wire[j+1] |= ((self.operations[i] & (0x0001 << j)) >> j << i) as u8;
                }
            }

            Ok(wire)
        }
    }

    fn from_wire(wire: &[u8]) -> Result<Self, api::ConversionError> {
        if wire.len() == 0 || (wire[0] < 16 && ((wire[0] + 1) as usize != wire.len())) || (wire[0] >= 16 && wire.len() != 17) {
            return Err(api::ConversionError::DeserialisationFailed("Invalid wire structure".to_owned()));
        }
        let mut npm = NodePropertyMap::new();
        if wire[0] < 16 {
            // List decode. Using internal access to avoid validation overhead.
            for &operation in &wire[1..] {
                if operation < 0x80 {
                    return Err(api::ConversionError::DeserialisationFailed(format!("Invalid operation '0x{:02x}' specified", operation)));
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
                return Err(api::ConversionError::DeserialisationFailed("Operation count mismatch in bitfield".to_owned()));
            }
            npm.operations_count = operations_count;
        }

        if wire[0] as usize == npm.operations_count {
            Ok(npm) 
        } else {
            Err(api::ConversionError::DeserialisationFailed("Incorrect number of properties set".to_owned()))
        } 
    }
}

/// Traits to represent the canonical data format. Vec<u8> is the generic form.
trait NodePropertyCanonicalType {}
impl NodePropertyCanonicalType for Vec<u8> {}
impl NodePropertyCanonicalType for bool {}
impl NodePropertyCanonicalType for chrono::NaiveDate {}
impl NodePropertyCanonicalType for String {}
impl NodePropertyCanonicalType for NodeEchonetLiteSupportedVersion {}
impl NodePropertyCanonicalType for NodePropertyMap {}
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

/// ECHONET Object representation (both device and profile)
struct NodeObject {
    eoj: api::EOJ,
    properties: Vec<Box<dyn NodePropertyGenericType>>
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

        // TODO: instead of having a canonical form of vec, should it be a string of hex, or in the HA adapter, do we convert from hex string to hex array?
        // Construct the node object
        NodeObject {
            eoj: api::EOJ::from_groupclass_instance(
                &api::CLASS_PROFILE_NODE_PROFILE,
                r#type as u8
            ),
            properties: vec![
                // Superclass specifications (6.10.1 Overview of Profile Object Super Class Specifications)
                Box::new(properties::NODE_PROPERTY_FAULT_STATUS.clone_with_canonical_value(&false).unwrap()),
                Box::new(properties::NODE_PROPERTY_MANUFACTURER_CODE.clone_with_internal_value(manufacturer_code).unwrap()),

                // Node profile class (6.11.1 Node Profile Class: Detailed Specifications)
                Box::new(properties::NODE_PROPERTY_OPERATING_STATUS.clone_with_canonical_value(&true).unwrap()),
                Box::new(properties::NODE_PROPERTY_VERSION_INFORMATION.clone_with_canonical_value(&NodeEchonetLiteSupportedVersion {
                    major_version: api::ECHONET_MAJOR_VERSION,
                    minor_version: api::ECHONET_MINOR_VERSION,
                    specified_message: true,
                    arbiturary_message: false
                }).unwrap()),
                Box::new(properties::NODE_PROPERTY_IDENTIFICATION_NUMBER.clone_with_internal_value(identification_number.as_slice()).unwrap()),
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
    fn unique_identifier(&self) -> Result<[u8; 16], api::EpcError> {
        let property_result = self.profile_object.get_node_property_by_template(&properties::NODE_PROPERTY_IDENTIFICATION_NUMBER);
        if let Ok(actual) = property_result {
            let data = actual.get_internal()?;
            Ok((&data[1..]).try_into()?)
        } else {    
            Err(api::EpcError::NotAvailable(properties::EPC_IDENTIFICATION_NUMBER))
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