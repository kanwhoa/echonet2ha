//! Middleware property unit tests
use super::*;
use super::super::{NodePropertyCanonicalType, api::EpcError};
use std::assert_matches;
use test_case::test_case;

/// Create a dummy Node Property for testing
fn create_node_property<T: NodePropertyCanonicalType + Default>() -> NodeProperty<T> {
    NodeProperty::new(
        "test", 
        0x00, 
        false, 
        NodePropertyOperation::Supported, 
        NodePropertyOperation::Supported, 
        |_, _| Ok(Vec::new()),
        |_, _| Ok(T::default()),
        |_, _| true
    )
}


#[test_case("", 0, &[] ; "when empty string and zero size")]
#[test_case("", 1, &[0x00] ; "when empty string and non zero size")]
#[test_case("ff", 1, &[0xFF] ; "when single byte")]
#[test_case("1234", 2, &[0x12, 0x34] ; "when two byte")]
#[test_case("f", 1, &[0x0F] ; "when 0.5 byte")]
#[test_case("234", 2, &[0x02, 0x34] ; "when 1.5 byte")]
#[test_case("0034", 1, &[0x34]; "when single byte padded")]
#[test_case("034", 1, &[0x34] ; "when 1.5 byte padded")]
fn from_hex_tests_success(input: &str, buf_len: usize, output: &[u8]) {
    let np = create_node_property::<String>();
    assert_matches!(from_hex(&np, input, buf_len, |_| true), Ok(v) if v == output.to_vec());
}

//#[test_case("1234", 1 ; "when single byte overflow")]
#[test_case("234", 1 ; "when 1.5 byte overflow")]
fn from_hex_tests_failure(input: &str, buf_len: usize) {
    let np = create_node_property::<String>();
    assert_matches!(from_hex(&np, input, buf_len, |_| true), Err(EpcError::InvalidValue(_)));
}

#[test_case(&[], 0, "" ; "when zero byte")]
#[test_case(&[], 1, "00" ; "when one byte padded")]
#[test_case(&[0x34], 1, "34" ; "when one byte")]
#[test_case(&[0x34], 2, "0034" ; "when two byte padded")]
#[test_case(&[0x12, 0x34], 2, "1234" ; "when two byte")]
fn to_hex_tests_success(input: &[u8], buf_len: usize, output: &str) {
    let np = create_node_property::<String>();
    assert_matches!(to_hex(&np, input, buf_len, |_| true), Ok(v) if v == output);
}

#[test_case(&[0x12, 0x34], 1 ; "when one byte overflow")]
fn to_hex_tests_failure(input: &[u8], buf_len: usize) {
    let np = create_node_property::<String>();
    assert_matches!(to_hex(&np, input, buf_len, |_| true), Err(EpcError::InvalidValue(_)));
}

#[test_case(&chrono::NaiveDate::from_ymd_opt(1999, 12, 20).unwrap(), &[0x07, 0xcf, 0x0c, 0x14] ; "when ECHONET example")]
fn from_date_tests_success(input: &chrono::NaiveDate, output: &[u8]) {
    let np = create_node_property::<chrono::NaiveDate>();
    assert_matches!(from_date(&np, input, |_| true), Ok(v) if v == output.to_vec());
}

#[test_case(&[0x07, 0xcf, 0x0c, 0x14], &chrono::NaiveDate::from_ymd_opt(1999, 12, 20).unwrap() ; "when ECHONET example")]
fn to_date_tests_success(input: &[u8], output: &chrono::NaiveDate) {
    let np = create_node_property::<chrono::NaiveDate>();
    assert_matches!(to_date(&np, input, |_| true), Ok(v) if &v == output);
}
