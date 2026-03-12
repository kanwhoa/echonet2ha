//! Middleware unit tests
use crate::middleware::api::{ConversionError, WirePresentable};

use super::*;
use std::{assert_eq, assert_matches};
use test_case::test_case;

#[test_case(&NodeEchonetLiteSupportedVersion{major_version: 1, minor_version: 14, specified_message: true, arbiturary_message: true}, &[0x01, 0x0e, 0x03, 0x00] ; "when 1.14 and both message types")]
#[test_case(&NodeEchonetLiteSupportedVersion{major_version: 1, minor_version: 14, specified_message: true, arbiturary_message: false}, &[0x01, 0x0e, 0x01, 0x00] ; "when 1.14 and only structured messages")]
#[test_case(&NodeEchonetLiteSupportedVersion{major_version: 1, minor_version: 14, specified_message: false, arbiturary_message: true}, &[0x01, 0x0e, 0x02, 0x00] ; "when 1.14 and only arbiturary messages")]
fn node_echonet_lite_supported_version_to_wire_tests_success(input: &NodeEchonetLiteSupportedVersion, output: &[u8]) {
    assert_matches!(input.to_wire(), Ok(v) if v == output.to_vec());
}

#[test_case(&NodeEchonetLiteSupportedVersion{major_version: 1, minor_version: 14, specified_message: false, arbiturary_message: false} ; "when 1.14 and no message types")]
fn node_echonet_lite_supported_version_to_wire_tests_failure(input: &NodeEchonetLiteSupportedVersion) {
    assert_matches!(input.to_wire(), Err(ConversionError::SerialisationFailed(_)));
}

#[test_case(&[0x80], &[0x01, 0x80] ; "when 1")]
#[test_case(&[0x80, 0x81, 0x8e, 0x8f, 0xf0, 0xf1, 0xfe, 0xff], &[0x08, 0x80, 0x81, 0x8e, 0x8f, 0xf0, 0xf1, 0xfe, 0xff] ; "when 8")]
#[test_case(&[0x80, 0x8f, 0x92, 0x9d, 0xa4, 0xab, 0xb6, 0xb9, 0xc6, 0xc9, 0xd4, 0xdb, 0xe2, 0xed, 0xf0, 0xff], &[0x10, 0x81, 0x00, 0x42, 0x00, 0x24, 0x00, 0x18, 0x00, 0x00, 0x18, 0x00, 0x24, 0x00, 0x42, 0x00, 0x81] ; "when 16")]
fn node_property_map_to_wire_tests_success(input: &[u8], output: &[u8]) {
    let mut properties = NodePropertyMap::new();
    for &operation in input {
        assert_matches!(properties.enable_operation(operation), Ok(_));
    }
    assert_matches!(properties.to_wire(), Ok(v) if v == output.to_vec());
}

#[test_case(&[0x01, 0x80], &[0x80] ; "when 1")]
#[test_case(&[0x08, 0x80, 0x81, 0x8e, 0x8f, 0xf0, 0xf1, 0xfe, 0xff], &[0x80, 0x81, 0x8e, 0x8f, 0xf0, 0xf1, 0xfe, 0xff] ; "when 8")]
#[test_case(&[0x10, 0x81, 0x00, 0x42, 0x00, 0x24, 0x00, 0x18, 0x00, 0x00, 0x18, 0x00, 0x24, 0x00, 0x42, 0x00, 0x81], &[0x80, 0x8f, 0x92, 0x9d, 0xa4, 0xab, 0xb6, 0xb9, 0xc6, 0xc9, 0xd4, 0xdb, 0xe2, 0xed, 0xf0, 0xff] ; "when 16")]
fn node_property_map_from_wire_tests_success(input: &[u8], output: &[u8]) {
    let result_properties = NodePropertyMap::from_wire(input);
    assert_matches!(result_properties, Ok(_));
    let properties = result_properties.unwrap();
    assert_eq!(properties.operations_count, input[0] as usize);

    for &operation in output {
        assert_matches!(properties.operation_enabled(operation), Ok(true), "operation '{:02x}' is not enabled", operation);
    }
}

