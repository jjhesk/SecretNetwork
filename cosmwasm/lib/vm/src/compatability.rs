use parity_wasm::elements::{Deserialize, Module};

use crate::errors::{Result, ValidationErr};

/// Lists all imports we provide upon instantiating the instance in Instance::from_module()
/// This should be updated when new imports are added
static SUPPORTED_IMPORTS: &[&str] = &[
    "env.read_db",
    "env.write_db",
    "env.canonicalize_address",
    "env.humanize_address",
];

/// Lists all entry points we expect to be present when calling a contract.
/// Basically, anything that is used in calls.rs
/// This is unlikely to change much, must be frozen at 1.0 to avoid breaking existing contracts
static REQUIRED_EXPORTS: &[&str] = &[
    "query",
    "init",
    "handle",
    "allocate",
    "deallocate",
    "cosmwasm_api_0_6",
];

pub fn check_api_compatibility(wasm_code: &[u8]) -> Result<()> {
    let mut reader = std::io::Cursor::new(wasm_code);
    let module = Module::deserialize(&mut reader).unwrap();
    if let Some(missing) = find_missing_import(&module, SUPPORTED_IMPORTS) {
        return ValidationErr {
            msg: format!(
                "Wasm contract requires unsupported import: \"{}\". Imports supported by VM: {:?}. Contract version too new for this VM?",
                missing, SUPPORTED_IMPORTS
            ),
        }
        .fail();
    }
    if let Some(missing) = find_missing_export(&module, REQUIRED_EXPORTS) {
        return ValidationErr {
            msg: format!(
                "Wasm contract doesn't have required export: \"{}\". Exports required by VM: {:?}. Contract version too old for this VM?",
                missing, REQUIRED_EXPORTS
            ),
        }
        .fail();
    }
    Ok(())
}

/// Checks if the import requirements of the contract are satisfied.
/// When this is not the case, we either have an incompatibility between contract and VM
/// or a error in the contract.
fn find_missing_import(module: &Module, supported_imports: &[&str]) -> Option<String> {
    let required_imports: Vec<String> = match module.import_section() {
        Some(import_section) => Vec::from(import_section.entries())
            .iter()
            .map(|entry| format!("{}.{}", entry.module(), entry.field()))
            .collect(),
        None => vec![],
    };

    for required_import in required_imports {
        if !supported_imports.contains(&required_import.as_str()) {
            return Some(required_import);
        }
    }
    None
}

fn find_missing_export(module: &Module, required_exports: &[&str]) -> Option<String> {
    let available_exports: Vec<String> = match module.export_section() {
        Some(export_section) => Vec::from(export_section.entries())
            .iter()
            .map(|entry| format!("{}", entry.field()))
            .collect(),
        None => vec![],
    };

    for required_export in required_exports {
        if !available_exports.iter().any(|x| x == required_export) {
            return Some(String::from(*required_export));
        }
    }
    None
}

#[cfg(test)]
mod test {
    use super::*;

    static CONTRACT_0_6: &[u8] = include_bytes!("../testdata/contract_0.6.wasm");
    static CONTRACT_0_7: &[u8] = include_bytes!("../testdata/contract_0.7.wasm");

    #[test]
    fn test_supported_imports() {
        let mut reader = std::io::Cursor::new(CONTRACT_0_6);
        let module = Module::deserialize(&mut reader).unwrap();

        // if contract has more than we provide, bad
        let imports_good = find_missing_import(&module, &["env.c_read", "env.c_write"]);
        assert_eq!(imports_good, Some(String::from("env.c_canonical_address")));

        // exact match good
        let imports_good = find_missing_import(
            &module,
            &[
                "env.c_read",
                "env.c_write",
                "env.c_canonical_address",
                "env.c_human_address",
            ],
        );
        assert_eq!(imports_good, None);

        // if we provide more, also good
        let imports_good = find_missing_import(
            &module,
            &[
                "env.c_read",
                "env.c_write",
                "env.c_canonical_address",
                "env.c_human_address",
                "env.future_function",
            ],
        );
        assert_eq!(imports_good, None);
    }

    #[test]
    fn test_required_exports() {
        let mut reader = std::io::Cursor::new(CONTRACT_0_6);
        let module = Module::deserialize(&mut reader).unwrap();

        // subset okay
        let exports_good = find_missing_export(&module, &["init", "handle", "allocate"]);
        assert_eq!(exports_good, None);

        // match okay
        let exports_good = find_missing_export(
            &module,
            &[
                "query",
                "init",
                "handle",
                "allocate",
                "deallocate",
                "cosmwasm_api_0_6",
            ],
        );
        assert_eq!(exports_good, None);

        // missing one from list not okay
        let missing_extra = find_missing_export(&module, &["init", "handle", "extra"]);
        assert_eq!(missing_extra, Some(String::from("extra")));
    }

    #[test]
    fn test_api_compatibility_imports() {
        use crate::errors::Error;

        // this is our reference check, must pass
        check_api_compatibility(CONTRACT_0_7).unwrap();

        // Old 0.6 contract rejected since it requires outdated imports `c_read` and friends
        match check_api_compatibility(CONTRACT_0_6) {
            Err(Error::ValidationErr { msg }) => {
                assert!(
                    msg.starts_with("Wasm contract requires unsupported import: \"env.c_read\"")
                );
            }
            Err(e) => panic!("Unexpected error {:?}", e),
            Ok(_) => panic!("Didn't reject wasm with invalid api"),
        }
    }

    #[test]
    fn test_api_compatibility_exports() {
        use crate::errors::Error;
        use wabt::wat2wasm;

        // this is invalid, as it doesn't contain all required exports
        static WAT_MISSING_EXPORTS: &'static str = r#"
            (module
              (type $t0 (func (param i32) (result i32)))
              (func $add_one (export "add_one") (type $t0) (param $p0 i32) (result i32)
                get_local $p0
                i32.const 1
                i32.add))
        "#;

        let wasm_missing_exports = wat2wasm(WAT_MISSING_EXPORTS).unwrap();

        match check_api_compatibility(&wasm_missing_exports) {
            Err(Error::ValidationErr { msg }) => {
                assert!(msg.starts_with("Wasm contract doesn't have required export: \"query\""));
            }
            Err(e) => panic!("Unexpected error {:?}", e),
            Ok(_) => panic!("Didn't reject wasm with invalid api"),
        }
    }
}
