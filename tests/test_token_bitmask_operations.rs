mod test_utils;

use serial_test::serial;
use test_utils::create_bitmask_dltensor;
use xgrammar::{DLTensor, allocate_token_bitmask, testing};

fn make_tensor_from_words(
    words: &[i32],
    vocab_size: usize,
) -> (DLTensor, Vec<i64>, Vec<i64>, Box<[i32]>) {
    let mut data = allocate_token_bitmask(1, vocab_size);
    for (i, &w) in words.iter().enumerate() {
        data[i] = w;
    }
    let (tensor, shape, strides) =
        create_bitmask_dltensor(&mut data, 1, vocab_size);
    (tensor, shape, strides, data)
}

#[test]
#[serial]
fn test_get_masked_tokens_from_bitmask() {
    let word: i32 = 0b0101_0011;
    let vocab_size = 8;
    let (tensor, _shape, _strides, _data) =
        make_tensor_from_words(&[word], vocab_size);
    let masked =
        testing::get_masked_tokens_from_bitmask(&tensor, vocab_size as i32, 0);
    let expected = vec![2, 3, 5, 7];
    assert_eq!(masked.as_ref(), expected);
}

#[test]
#[serial]
fn test_get_masked_tokens_from_bitmask_all_allowed() {
    let vocab_size = 10;
    let (tensor, _shape, _strides, _data) =
        make_tensor_from_words(&[-1i32], vocab_size);
    let masked =
        testing::get_masked_tokens_from_bitmask(&tensor, vocab_size as i32, 0);
    assert!(masked.is_empty());
}

#[test]
#[serial]
fn test_is_single_token_bitmask() {
    let vocab_size = 16;
    let (tensor, _shape, _strides, _data) =
        make_tensor_from_words(&[0], vocab_size);
    assert_eq!(
        testing::is_single_token_bitmask(&tensor, vocab_size as i32, 0),
        (false, -1)
    );

    let (tensor_single, _shape1, _strides1, _data1) =
        make_tensor_from_words(&[1 << 5], vocab_size);
    assert_eq!(
        testing::is_single_token_bitmask(&tensor_single, vocab_size as i32, 0),
        (true, 5)
    );

    let (tensor_multi, _shape2, _strides2, _data2) =
        make_tensor_from_words(&[(1 << 2) | (1 << 7)], vocab_size);
    assert_eq!(
        testing::is_single_token_bitmask(&tensor_multi, vocab_size as i32, 0),
        (false, -1)
    );
}
