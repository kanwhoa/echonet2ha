//! Middleware implementation for EchoNet Lite implementation
//! This is a relatively light middleware. The bridge is not currently two way,
//! I.e. it publishes EchoNet Lite objects and their profiles to MQTT, but does
//! not consume MQTT queues and publish back. Therefore, the actual number of
//! instances (exluding node profile objects) is minimal, as this doesn't really
//! do anything. Additionaly, this middleware is not functioning as a controller
//! or HEMS, so should not have properties. Ideally something like Home Assistant
//! is decising what actions to take and so all devices on the ECHONET Lite
//! network should be treated as nodes to be controlled. We will not create a 
//! dummy controller entry for the Home Assistant device, since that is optional.
use lazy_static::lazy_static;
use std::collections::HashMap;

// INFO ON DEVICES AND PUBLISHED INFO:
// https://echonet.jp/wp/wp-content/uploads/pdf/General/Standard/Release/Release_Q/Appendix_Release_Q_E.pdf

/// ECHONET Lite Object Specification (addressing)
/// A node can contain multiple objects which are addressable through the "ECHONET Lite Object Spefification" (EOJ)
/// * Device Objects. These contain state and properties as per "APPENDIX Detailed Requirements for ECHONET Device objects"
/// * Profile Objects. These
struct EOJ {
    class_group_code: u8, // E.g. sensors, home equipment, etc
    class_code: u8, // The specific type, e.g. a presence sensor
    instance_code: u8 // The instance number of the presence sensor, for example devices that have both PIR and mmWave 
}

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
// Specfic constants for the node Profile Object class.
const INSTANCE_GENERAL_NODE: u8 = 0x01;
const INSTANCE_TX_ONLY_NODE: u8 = 0x02;

// Create a map of all the known/handled groups and codes.
// FIXME: do this by a enum/struct/match?
// This is to build a copy of the states/properties advertised by other devices.
lazy_static! {
    static ref EPC_HANDLERS: HashMap<u8, HashMap<u8, &'static str>> = {
        let mut groups = HashMap::new();

        // Group: Profile
        {
            let mut classes = HashMap::new();

            // Class: Node Profile
            classes.insert(0xf0, "Done?");

            groups.insert(EOJ_CLASS_GROUP_PROFILE, classes);
        }

        // Finished
        groups
    };
}

// Need to think about how we create our own middleware implementation for the controller object. Only need to report. The operation status,
// But ideally other properties about the devices under control might be nice.

// 
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

// TODO: determine how to the recieved profile state, and how to trigger an update to MQTT.
// This likely needs a multithreadded concept. Can send an IPC reques tto eh MQTT handler to update/post a single message
// That means two event handler threas, one for MQTT, one for ECHONET Lite network traffic?
// Some sort of loop: wait for packet -> middleware handle (event change update to other thread) -> outbound/notify packet. Similarly, this needs
// to also check the event thread for updates which need to then be pushed back.


// Notes: use Tokio oneshot to handle IPC/threads.