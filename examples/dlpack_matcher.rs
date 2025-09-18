use autocxx::{WithinBox, cxx};
use xgrammar::{
    cxx_utils,
    xgrammar::{
        GetBitmaskDLType, GetBitmaskSize, GrammarCompiler, TokenizerInfo,
    },
};

fn main() {
    // 1) Build tokenizer info and compiler
    let vocab: Vec<String> = vec![
        "{".to_string(),
        "}".to_string(),
        "[".to_string(),
        "]".to_string(),
        ",".to_string(),
        ":".to_string(),
        "\"".to_string(),
        "0".to_string(),
        "1".to_string(),
        "2".to_string(),
    ];

    let mut encoded_vocab = cxx_utils::new_string_vector();
    {
        let mut vpin = encoded_vocab.pin_mut();
        cxx_utils::string_vec_reserve(vpin.as_mut(), vocab.len());
        for item in &vocab {
            let bytes = item.as_bytes();
            unsafe {
                cxx_utils::string_vec_push_bytes(
                    vpin.as_mut(),
                    bytes.as_ptr() as *const i8,
                    bytes.len(),
                );
            }
        }
    }
    let meta = format!(
        "{{\"vocab_type\":0,\"vocab_size\":{},\"add_prefix_space\":false,\"stop_token_ids\":[]}}",
        vocab.len()
    );
    cxx::let_cxx_string!(metadata = meta);

    let tok = TokenizerInfo::FromVocabAndMetadata(
        encoded_vocab.as_ref().unwrap(),
        &metadata,
    )
    .within_box();

    let mut compiler = GrammarCompiler::new(
        &tok,
        autocxx::c_int(4),
        true,
        autocxx::c_longlong(-1),
    )
    .within_box();
    let compiled = compiler.as_mut().CompileBuiltinJSONGrammar().within_box();

    // Create matcher via helper
    let mut matcher =
        cxx_utils::make_grammar_matcher(&compiled, false, autocxx::c_int(64))
            .within_box();

    // Build a DLTensor bitmask for next-token constraints
    let vocab_size_c = tok.GetVocabSize();
    let buffer_len = GetBitmaskSize(vocab_size_c) as usize;
    let mut storage = vec![0i32; buffer_len];

    // Prepare DLTensor view over storage
    let mut bm_shape: i64 = buffer_len as i64;
    let mut bm_stride: i64 = 1;
    let mut bitmask = xgrammar::DLTensor {
        data: storage.as_mut_ptr() as *mut core::ffi::c_void,
        device: xgrammar::DLDevice {
            device_type: xgrammar::DLDeviceType::kDLCPU,
            device_id: 0,
        },
        ndim: 1,
        dtype: GetBitmaskDLType(),
        shape: &mut bm_shape as *mut i64,
        strides: &mut bm_stride as *mut i64,
        byte_offset: 0,
    };

    // Fill bitmask at index 0
    let need_apply = unsafe {
        cxx_utils::matcher_fill_next_token_bitmask(
            matcher.as_mut(),
            &mut bitmask as *mut xgrammar::DLTensor,
            autocxx::c_int(0),
            false,
        )
    };
    println!("need_apply={}", need_apply);
    // Build logits tensor (float32)
    let vocab_size_usize: usize = vocab.len();
    let vocab_size_i64: i64 = vocab_size_usize as i64;
    let mut logits = vec![0f32; buffer_len.max(vocab_size_usize)];
    let mut lg_shape: i64 = vocab_size_i64;
    let mut lg_stride: i64 = 1;
    let mut logits_tensor = xgrammar::DLTensor {
        data: logits.as_mut_ptr() as *mut core::ffi::c_void,
        device: xgrammar::DLDevice {
            device_type: xgrammar::DLDeviceType::kDLCPU,
            device_id: 0,
        },
        ndim: 1,
        dtype: xgrammar::DLDataType {
            code: 2,
            bits: 32,
            lanes: 1,
        },
        shape: &mut lg_shape as *mut i64,
        strides: &mut lg_stride as *mut i64,
        byte_offset: 0,
    };

    // Apply mask to logits (in-place)
    unsafe {
        cxx_utils::apply_token_bitmask_inplace_cpu(
            &mut logits_tensor as *mut xgrammar::DLTensor,
            &bitmask,
            vocab_size_c,
        )
    };
    println!("ok; buffer_len={} elements", buffer_len);
}
