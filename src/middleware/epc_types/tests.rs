//! API test cases
use super::*;
use std::assert_matches;
use test_case::test_case;
use super::EPC_PRODUCTION_DATE;
use super::super::api::{CLASS_PROFILE_NODE_PROFILE, CLASS_CONTROL_CONTROLLER};

/// A general property test
fn property_test<T>(group_class: &NodeGroupClass, epc: u8, input: &T, internal: &[u8])
where
    T: Debug + Display + 'static
{
    // Create the EPC
    let epc = property_factory(group_class, epc)
        .unwrap_or_else(|err| panic!("failed to create property: {}", err));

    // Downcast to get the original type out.
    let Some(epc_real) = epc.as_any().downcast_ref::<StaticNodeProperty<T>>() else {
        panic!("Downcast failed");
    };

    // Make sure that the internal value is correct
    epc_real.from_canonical(input).unwrap_or_else(|err| panic!("Unable to set canonical value: {}", err));
    assert_matches!(epc_real.get(), Ok(val) if val == internal);

    // Update the internal value and re-assert that it is still the same.
    epc_real.set(internal).unwrap_or_else(|err| panic!("Unable to set internal value: {}", err));
    assert_matches!(epc_real.get(), Ok(val) if val == internal);
}

#[test_case(&chrono::NaiveDate::from_ymd_opt(1999, 12, 20).unwrap(), &[0x07, 0xcf, 0x0c, 0x14] ; "when ECHONET example")]
fn from_date_tests_success(input: &chrono::NaiveDate, internal: &[u8]) {
    property_test(
        &CLASS_PROFILE_NODE_PROFILE,
        EPC_PRODUCTION_DATE,
        input,
        internal
    );
}


#[test_case(&NodeProfileObjectEchonetLiteSupportedVersion{major_version: 1, minor_version: 14, specified_message: true, arbiturary_message: true}, &[0x01, 0x0e, 0x03, 0x00] ; "when 1.14 and both message types")]
#[test_case(&NodeProfileObjectEchonetLiteSupportedVersion{major_version: 1, minor_version: 14, specified_message: true, arbiturary_message: false}, &[0x01, 0x0e, 0x01, 0x00] ; "when 1.14 and only structured messages")]
#[test_case(&NodeProfileObjectEchonetLiteSupportedVersion{major_version: 1, minor_version: 14, specified_message: false, arbiturary_message: true}, &[0x01, 0x0e, 0x02, 0x00] ; "when 1.14 and only arbiturary messages")]
fn node_profile_object_echonet_lite_supported_version_to_wire_tests_success(input: &NodeProfileObjectEchonetLiteSupportedVersion, internal: &[u8]) {
    property_test(
        &CLASS_PROFILE_NODE_PROFILE,
        EPC_VERSION_INFORMATION,
        input,
        internal
    );
}

#[test_case(&NodeDeviceObjectEchonetLiteSupportedVersion{release: 'Q', revision: 0x01}, &[0x00, 0x00, 0x51, 0x01] ; "when release q, revision 1, uppercase")]
#[test_case(&NodeDeviceObjectEchonetLiteSupportedVersion{release: 'q', revision: 0x01}, &[0x00, 0x00, 0x51, 0x01] ; "when release q, revision 1, lowercase")]
fn node_device_object_echonet_lite_supported_version_to_wire_tests_success(input: &NodeDeviceObjectEchonetLiteSupportedVersion, internal: &[u8]) {
    property_test(
        &CLASS_CONTROL_CONTROLLER,
        EPC_VERSION_INFORMATION,
        input,
        internal
    );
}


/*
FIXME: figure out how to pass the error struct
#[test_case(&NodeProfileObjectEchonetLiteSupportedVersion{major_version: 1, minor_version: 14, specified_message: false, arbiturary_message: false} ; "when 1.14 and no message types")]
fn node_profile_object_echonet_lite_supported_version_to_wire_tests_failure(input: &NodeProfileObjectEchonetLiteSupportedVersion) {
    assert_matches!(input.to_wire(), Err(ConversionError::SerialisationFailed(_)));
}
    */