//! API test cases
use super::*;
use std::assert_matches;
use test_case::test_case;


#[test_case("", &[] ; "when empty string and zero size")]
#[test_case("ff", &[0xFF] ; "when single byte")]
#[test_case("1234", &[0x12, 0x34] ; "when two byte")]
#[test_case("f", &[0x0F] ; "when 0.5 byte")]
#[test_case("234", &[0x02, 0x34] ; "when 1.5 byte")]
#[test_case("0034", &[0x00, 0x34]; "when 1 byte padded")]
#[test_case("034", &[0x00, 0x34] ; "when 1.5 byte padded")]
fn hexstring_from_str_tests_success(input: &str, output: &[u8]) {
    let hs = HexString::new(input);
    assert_matches!(hs, Ok(_));
    assert_eq!(hs.unwrap().decode(), output.to_vec());
}

#[test_case("xx00" ; "when invalid char double")]
#[test_case("0x00" ; "when invalid char single")]
fn hexstring_from_str_tests_failure(input: &str) {
    let hs = HexString::new(input);
    assert_matches!(hs, Err(HexStringError::InvalidCharacter));
}

#[test_case("", 1, &[0x00] ; "when empty and one byte pad")]
#[test_case("ff", 5, &[0x00, 0x00, 0x00, 0x00, 0xff] ; "when single byte and four byte pad")]
#[test_case("0034", 1, &[0x34]; "when 1 byte padded")]
fn hexstring_from_str_tests_success_sized(input: &str, buf_len: usize, output: &[u8]) {
    let hs = HexString::new(input);
    assert_matches!(hs, Ok(_));
    
    let mut buf = vec!(0x00_u8; buf_len);
    assert_matches!(hs.unwrap().decode_into_slice(buf.as_mut_slice()), Ok(()));
    assert_eq!(&buf[..], output)
}

#[test_case("12", 0 ; "when 1 byte with overflow")]
#[test_case("1234", 1 ; "when 1.5 bytes with overflow")]
#[test_case("1234", 1 ; "when 2 bytes with overflow")]
fn hexstring_from_str_tests_failure_sized(input: &str, buf_len: usize) {
    let hs = HexString::new(input);
    assert_matches!(hs, Ok(_));
    
    let mut buf = vec!(0x00_u8; buf_len);
    assert_matches!(hs.unwrap().decode_into_slice(buf.as_mut_slice()), Err(HexStringError::BufferTooSmall));
}

#[test_case(&[], "" ; "when 0 byte")]
#[test_case(&[0xff], "ff" ; "when 1 byte lowercase")]
#[test_case(&[0xff], "FF" ; "when 1 byte uppercase")]
#[test_case(&[0x00, 0x34], "0034" ; "when zero padded 2 byte")]
#[test_case(&[0x12, 0x34], "1234" ; "when 2 byte")]
fn hexstring_from_bytes_tests_success(input: &[u8], output: &str) {
    let hs: HexString = input.into();
    assert_eq!(hs, output);
}

#[test_case(&[], 1, "00" ; "when 0 byte with len 1")]
#[test_case(&[0x00], 0, "" ; "when only padding and zero byte")]
#[test_case(&[0x12], 2, "0012" ; "when 1 byte with len 2")]
#[test_case(&[0x00, 0x12], 1, "12" ; "when 1 byte with 1 byte padding and len 1")]
#[test_case(&[0x00, 0x12, 0x34], 2, "1234" ; "when 2 byte with 1 byte padding and len 2")]
fn hexstring_from_bytes_tests_success_sized(input: &[u8], len: usize, output: &str) {
    let hs = HexString::from_bytes(input, len);
    assert_matches!(hs, Ok(val) if val == output);
}

#[test_case(&[0x34], 0 ; "when 1 byte buf with 0 byte len")]
#[test_case(&[0x12, 0x34], 1 ; "when 2 byte buf with 1 byte len")]
fn hexstring_from_bytes_tests_failure_sized(input: &[u8], len: usize) {
    let hs = HexString::from_bytes(input, len);
    assert_matches!(hs, Err(HexStringError::BufferTooSmall));
}

#[test_case(&[0x80], &[0x01, 0x80] ; "when 1")]
#[test_case(&[0x80, 0x81, 0x8e, 0x8f, 0xf0, 0xf1, 0xfe, 0xff], &[0x08, 0x80, 0x81, 0x8e, 0x8f, 0xf0, 0xf1, 0xfe, 0xff] ; "when 8")]
#[test_case(&[0x80, 0x8f, 0x92, 0x9d, 0xa4, 0xab, 0xb6, 0xb9, 0xc6, 0xc9, 0xd4, 0xdb, 0xe2, 0xed, 0xf0, 0xff], &[0x10, 0x81, 0x00, 0x42, 0x00, 0x24, 0x00, 0x18, 0x00, 0x00, 0x18, 0x00, 0x24, 0x00, 0x42, 0x00, 0x81] ; "when 16")]
fn node_property_map_serialise_tests_success(input: &[u8], output: &[u8]) {
    let mut properties = NodeObjectPropertyMap::new();
    for &operation in input {
        assert_matches!(properties.enable_operation(operation), Ok(_));
    }
    assert_eq!(properties.decode(), output.to_vec());
}

#[test_case(&[0x01, 0x80], &[0x80] ; "when 1")]
#[test_case(&[0x08, 0x80, 0x81, 0x8e, 0x8f, 0xf0, 0xf1, 0xfe, 0xff], &[0x80, 0x81, 0x8e, 0x8f, 0xf0, 0xf1, 0xfe, 0xff] ; "when 8")]
#[test_case(&[0x10, 0x81, 0x00, 0x42, 0x00, 0x24, 0x00, 0x18, 0x00, 0x00, 0x18, 0x00, 0x24, 0x00, 0x42, 0x00, 0x81], &[0x80, 0x8f, 0x92, 0x9d, 0xa4, 0xab, 0xb6, 0xb9, 0xc6, 0xc9, 0xd4, 0xdb, 0xe2, 0xed, 0xf0, 0xff] ; "when 16")]
fn node_property_map_deserialise_tests_success(input: &[u8], output: &[u8]) {
    let result_properties = NodeObjectPropertyMap::from_bytes(input);
    assert_matches!(result_properties, Ok(_));
    let properties = result_properties.unwrap();
    assert_eq!(properties.operations_count, input[0] as usize);

    for &operation in output {
        assert_matches!(properties.is_operation_enabled(operation), Ok(true), "operation '{:02x}' is not enabled", operation);
    }
}

