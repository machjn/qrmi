// This code is part of Qiskit.
//
// (C) Copyright Alice and Bob 2026
//
// This code is licensed under the Apache License, Version 2.0. You may
// obtain a copy of this license in the LICENSE.txt file in the root directory
// of this source tree or at http://www.apache.org/licenses/LICENSE-2.0.
//
// Any modifications or derivative works of this code must retain this
// copyright notice, and modified files need to carry a notice indicating
// that they have been altered from the originals.

use super::AliceBobFelis;
use crate::QrmiError;
use std::collections::HashMap;

fn config_with_api_key(api_key: &str) -> HashMap<String, String> {
    HashMap::from([
        ("QRMI_AB_FELIS_API_KEY".to_string(), api_key.to_string()),
        (
            "QRMI_AB_FELIS_BASE_ENDPOINT".to_string(),
            "https://api.example.com".to_string(),
        ),
    ])
}

#[test]
fn from_config_accepts_valid_api_key() {
    // base64("user:password")
    AliceBobFelis::from_config("ab_emu", config_with_api_key("dXNlcjpwYXNzd29yZA=="))
        .expect("from_config should succeed");
}

#[test]
fn from_config_rejects_invalid_api_key_without_leaking_it() {
    let api_key = "not-a-base64-key"; // pragma: allowlist secret
    let err = AliceBobFelis::from_config("ab_emu", config_with_api_key(api_key))
        .map(|_| ())
        .unwrap_err();
    assert!(
        matches!(err, QrmiError::InvalidConfig(_)),
        "unexpected error: {err:?}"
    );
    assert!(
        !err.to_string().contains(api_key),
        "error leaks the API key: {err}"
    );
}
