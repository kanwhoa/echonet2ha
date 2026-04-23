//! API test cases
use super::*;
use std::assert_matches;
use test_case::test_case;
use super::EPC_PRODUCTION_DATE;
use super::super::api::{CLASS_PROFILE_NODE_PROFILE, CLASS_CONTROL_CONTROLLER};
use super::super::api::{EOJ_CLASS_GROUP_CONTROL};

/// A general property test.
/// The internal value is only the actual data, i.e. not including the EPC header
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

/*
FIXME: figure out how to pass the error struct
#[test_case(&NodeProfileObjectEchonetLiteSupportedVersion{major_version: 1, minor_version: 14, specified_message: false, arbiturary_message: false} ; "when 1.14 and no message types")]
fn node_profile_object_echonet_lite_supported_version_to_wire_tests_failure(input: &NodeProfileObjectEchonetLiteSupportedVersion) {
    assert_matches!(input.to_wire(), Err(ConversionError::SerialisationFailed(_)));
}
    */

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


#[test_case(&NodeObjectInstanceCount(0x012345), &[0x01, 0x23, 0x45] ; "when valid")]
#[test_case(&NodeObjectInstanceCount(0x01ffffff), &[0xff, 0xff, 0xff] ; "when overflow")]
fn node_object_instance_count_to_wire_tests_success(input: &NodeObjectInstanceCount, internal: &[u8]) {
    property_test(
        &CLASS_PROFILE_NODE_PROFILE,
        EPC_NUMBER_OF_SELFNODE_INSTANCES,
        input,
        internal
    );
}


#[test_case(&EOJVec(vec![]), &[] ; "when 0")]
#[test_case(&EOJVec(vec![EOJ::from_groupclass_instance(&CLASS_CONTROL_CONTROLLER, 0x01)]), &[EOJ_CLASS_GROUP_CONTROL, 0xff, 0x01] ; "when 1")]
#[test_case(&EOJVec(vec![EOJ::from_groupclass_instance(&CLASS_CONTROL_CONTROLLER, 0x01), EOJ::from_groupclass_instance(&CLASS_CONTROL_CONTROLLER, 0x02)]), &[EOJ_CLASS_GROUP_CONTROL, 0xff, 0x01, EOJ_CLASS_GROUP_CONTROL, 0xff, 0x02] ; "when 2")]
fn node_profile_object_announce_list_to_wire_tests_success(input: &EOJVec, internal: &[u8]) {
    property_test(
        &CLASS_PROFILE_NODE_PROFILE,
        EPC_INSTANCE_LIST_NOTIFICATION,
        input,
        internal
    );
}


#[test_case(0.0, &[0x00] ; "when 0%")]
#[test_case(50.0, &[0x32] ; "when 50%")]
#[test_case(100.0, &[0x64] ; "when 100%")]
#[test_case(f64::NEG_INFINITY, &[0xfe] ; "when underflow")]
#[test_case(f64::INFINITY, &[0xff] ; "when overflow")]
fn from_percentage_success(input: f64, internal: &[u8]) {
    property_test(
        &CLASS_CONTROL_CONTROLLER,
        EPC_CURRENT_LIMIT_SETTING,
        &input,
        internal
    );
}
// TODO: failure tests


#[test_case(0.0, &[0x00, 0x00, 0x00, 0x00] ; "when 0.000w")]
#[test_case(0.001, &[0x00, 0x00, 0x00, 0x01] ; "when 0.001w")]
#[test_case(1.0, &[0x00, 0x00, 0x03, 0xe8] ; "when 1w")]
#[test_case(999_999.999, &[0x3b, 0x9a, 0xc9, 0xff] ; "when 999,999.999w")]
#[test_case(f64::NEG_INFINITY, &[0xff, 0xff, 0xff, 0xfe] ; "when underflow")]
#[test_case(f64::INFINITY, &[0xff, 0xff, 0xff, 0xff] ; "when overflow")]
fn from_float_success(input: f64, internal: &[u8]) {
    property_test(
        &CLASS_PROFILE_NODE_PROFILE,
        EPC_MEASURED_CUMULATIVE_POWER_CONSUMPTION,
        &input,
        internal
    );
}
// TODO: failure tests


// TODO tests:
// EPC_FAULT_CONTENT -> NodeObjectFaultDescription
// ? -> NodeObjectInstallationLocation
// > -> time property
// > -> duration property