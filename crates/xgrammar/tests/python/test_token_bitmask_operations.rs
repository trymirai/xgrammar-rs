//! Port of `xgrammar/tests/python/test_token_bitmask_operations.py`.
//!
//! The pure (non-tensor) bitmask operations are ported here. The GPU/triton/metal kernel
//! variants are CPU-only N/A; the torch/DLTensor wrappers belong to the bindings layer.

use xgrammar::matcher::{
    allocate_token_bitmask, apply_token_bitmask_inplace_cpu, get_bitmask_size,
    get_masked_tokens_from_bitmask, is_single_token_bitmask,
    reset_token_bitmask,
};

/// Packs a `batch × vocab_size` boolean mask (true = allowed) into the i32 bitmask layout.
fn bool_mask_to_bitmask(
    bool_mask: &[Vec<bool>],
    vocab_size: i32,
) -> Vec<i32> {
    let size = get_bitmask_size(vocab_size) as usize;
    let mut bitmask = vec![0i32; bool_mask.len() * size];
    for (b, row) in bool_mask.iter().enumerate() {
        for (token, &allowed) in row.iter().enumerate() {
            if allowed {
                bitmask[b * size + token / 32] |= 1 << (token % 32);
            }
        }
    }
    bitmask
}

#[test]
fn test_allocate_reset_token_bitmask() {
    let batch_size = 10;
    let vocab_size = 128_005;
    let mut bitmask = allocate_token_bitmask(batch_size, vocab_size);
    assert_eq!(bitmask.len(), (batch_size * ((vocab_size + 31) / 32)) as usize);
    assert!(bitmask.iter().all(|&w| w == -1));
    bitmask.fill(0);
    reset_token_bitmask(&mut bitmask);
    assert!(bitmask.iter().all(|&w| w == -1));
}

#[test]
fn test_get_masked_tokens_from_bitmask() {
    for token_mask_size in [1024, 32000, 32001, 32011] {
        for index in [0, 1] {
            // A deterministic pseudo-random bool mask (true = allowed).
            let bool_mask: Vec<Vec<bool>> = (0..2)
                .map(|r| {
                    (0..token_mask_size)
                        .map(|t| {
                            ((t as i64 * 1103515245 + r as i64 * 12345 + 7)
                                >> 5)
                                & 1
                                == 0
                        })
                        .collect()
                })
                .collect();
            let bitmask = bool_mask_to_bitmask(&bool_mask, token_mask_size);
            let expected: Vec<i32> = (0..token_mask_size)
                .filter(|&t| !bool_mask[index as usize][t as usize])
                .collect();
            assert_eq!(
                get_masked_tokens_from_bitmask(
                    &bitmask,
                    token_mask_size,
                    index
                ),
                expected
            );
        }
    }
}

#[test]
fn test_is_single_token_bitmask() {
    let vocab_size = 1024;
    let batch_index = 1;
    let token_id = 100;

    let mut bool_mask = vec![vec![false; vocab_size as usize]; 2];
    let bitmask = bool_mask_to_bitmask(&bool_mask, vocab_size);
    assert_eq!(
        is_single_token_bitmask(&bitmask, vocab_size, batch_index),
        (false, -1)
    );

    bool_mask[batch_index as usize][token_id as usize] = true;
    let bitmask = bool_mask_to_bitmask(&bool_mask, vocab_size);
    assert_eq!(
        is_single_token_bitmask(&bitmask, vocab_size, batch_index),
        (true, token_id)
    );

    bool_mask[batch_index as usize][(token_id + 1) as usize] = true;
    let bitmask = bool_mask_to_bitmask(&bool_mask, vocab_size);
    assert_eq!(
        is_single_token_bitmask(&bitmask, vocab_size, batch_index),
        (false, -1)
    );
}

#[test]
fn test_apply_token_bitmask_inplace_cpu() {
    let bitmask = [0b10_1010_1010];
    let mut logits = [1.0f32, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
    apply_token_bitmask_inplace_cpu(&mut logits, &bitmask, 10);
    let neg = f32::NEG_INFINITY;
    assert_eq!(logits, [neg, 2.0, neg, 4.0, neg, 6.0, neg, 8.0, neg, 10.0]);
}
