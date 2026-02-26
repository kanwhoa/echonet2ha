//! Converters module.
//! These convert a middleware message into a specific output format
//! i.e. ECHONET or Home Assistant. Not all events may be relevant
//! for the connector. So for example, a message has been converted
//! the connector should decide if the message is relevant by
//! by looking at the target of the message (single device or broadcast). 
//pub mod echonet;
//pub mod homeassistant;