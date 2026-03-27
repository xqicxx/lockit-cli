//! Biometric unlock integration.
//!
//! On macOS, the vault password is stored in the login Keychain with a
//! `kSecAccessControlBiometryAny` access-control flag so that every retrieval
//! requires a Touch ID (or Face ID) challenge — or the device passcode as a
//! fallback.  The password itself is never written to disk in plaintext.
//!
//! On non-macOS platforms the functions return a descriptive error so callers
//! can offer a graceful fallback to the interactive password prompt.
//!
//! ## User-facing flow
//!
//! ```text
//! # First-time setup — save vault password to Keychain under Touch ID
//! lk unlock --save-biometric
//!
//! # Subsequent unlocks — Touch ID prompt appears automatically
//! lk unlock --biometric
//! ```

// ─── Platform-specific implementations ────────────────────────────────────────

#[cfg(target_os = "macos")]
mod macos_impl {
    use anyhow::{Result, bail};
    use core_foundation::{
        base::{CFType, TCFType, kCFAllocatorDefault},
        boolean::CFBoolean,
        data::CFData,
        dictionary::CFMutableDictionary,
        string::CFString,
    };
    use secrecy::SecretString;
    use std::ptr;
    use zeroize::Zeroize;

    // Keychain item identity.
    const SERVICE: &str = "lockit";
    const ACCOUNT: &str = "vault-master-password";
    // Human-readable reason shown in the Touch ID prompt.
    const REASON: &str = "Unlock your lockit vault";

    // ── Raw Security framework constants ──────────────────────────────────────

    #[link(name = "Security", kind = "framework")]
    unsafe extern "C" {
        static kSecClass: *const std::ffi::c_void;
        static kSecClassGenericPassword: *const std::ffi::c_void;
        static kSecAttrService: *const std::ffi::c_void;
        static kSecAttrAccount: *const std::ffi::c_void;
        static kSecAttrAccessControl: *const std::ffi::c_void;
        static kSecValueData: *const std::ffi::c_void;
        static kSecReturnData: *const std::ffi::c_void;
        static kSecMatchLimit: *const std::ffi::c_void;
        static kSecMatchLimitOne: *const std::ffi::c_void;
        static kSecUseOperationPrompt: *const std::ffi::c_void;

        fn SecItemAdd(
            attributes: core_foundation::dictionary::CFDictionaryRef,
            result: *mut core_foundation::base::CFTypeRef,
        ) -> i32;

        fn SecItemCopyMatching(
            query: core_foundation::dictionary::CFDictionaryRef,
            result: *mut core_foundation::base::CFTypeRef,
        ) -> i32;

        fn SecItemDelete(query: core_foundation::dictionary::CFDictionaryRef) -> i32;

        fn SecAccessControlCreateWithFlags(
            allocator: core_foundation::base::CFAllocatorRef,
            protection: core_foundation::base::CFTypeRef,
            flags: u64,
            error: *mut core_foundation::base::CFTypeRef,
        ) -> *mut std::ffi::c_void;
    }

    // kSecAttrAccessibleWhenUnlockedThisDeviceOnly
    #[link(name = "Security", kind = "framework")]
    unsafe extern "C" {
        static kSecAttrAccessibleWhenUnlockedThisDeviceOnly: *const std::ffi::c_void;
    }

    // SecAccessControlCreateFlags constants
    const SEC_ACCESS_CONTROL_BIOMETRY_ANY: u64 = 1 << 1; // kSecAccessControlBiometryAny
    const SEC_ACCESS_CONTROL_OR: u64 = 1 << 14; // kSecAccessControlOr
    const SEC_ACCESS_CONTROL_DEVICE_PASSCODE: u64 = 1 << 4; // kSecAccessControlDevicePasscode

    // SecItemDelete / SecItemAdd / SecItemCopyMatching return codes.
    const ERR_SEC_SUCCESS: i32 = 0;
    const ERR_SEC_ITEM_NOT_FOUND: i32 = -25300;
    const ERR_SEC_DUPLICATE_ITEM: i32 = -25299;

    // ── Public API ────────────────────────────────────────────────────────────

    /// Save `password` to the macOS Keychain under a biometric access-control
    /// policy (`kSecAccessControlBiometryAny | kSecAccessControlOr |
    /// kSecAccessControlDevicePasscode`).
    ///
    /// Any existing Keychain item for the same service/account is replaced.
    pub fn save_password(password: &str) -> Result<()> {
        // Delete any previous entry (ignore "not found").
        delete_item();

        let access_control = create_biometric_access_control()?;

        let service = CFString::new(SERVICE);
        let account = CFString::new(ACCOUNT);
        let data = CFData::from_buffer(password.as_bytes());

        unsafe {
            let mut dict = CFMutableDictionary::new();

            // kSecClass = kSecClassGenericPassword
            dict.set(
                CFType::wrap_under_get_rule(kSecClass as *const _),
                CFType::wrap_under_get_rule(kSecClassGenericPassword as *const _),
            );
            dict.set(
                CFType::wrap_under_get_rule(kSecAttrService as *const _),
                service.as_CFType(),
            );
            dict.set(
                CFType::wrap_under_get_rule(kSecAttrAccount as *const _),
                account.as_CFType(),
            );
            dict.set(
                CFType::wrap_under_get_rule(kSecAttrAccessControl as *const _),
                CFType::wrap_under_get_rule(access_control as *const _),
            );
            dict.set(
                CFType::wrap_under_get_rule(kSecValueData as *const _),
                data.as_CFType(),
            );

            let rc = SecItemAdd(dict.as_concrete_TypeRef(), ptr::null_mut());
            if rc == ERR_SEC_DUPLICATE_ITEM {
                // Retry once after deleting.
                delete_item();
                let rc2 = SecItemAdd(dict.as_concrete_TypeRef(), ptr::null_mut());
                if rc2 != ERR_SEC_SUCCESS {
                    bail!(
                        "Failed to save to Keychain (SecItemAdd code {}). \
                         Make sure Touch ID is configured on this Mac.",
                        rc2
                    );
                }
            } else if rc != ERR_SEC_SUCCESS {
                bail!(
                    "Failed to save to Keychain (SecItemAdd code {}). \
                     Make sure Touch ID is configured on this Mac.",
                    rc
                );
            }
        }
        Ok(())
    }

    /// Retrieve the vault password from the Keychain.  macOS will show a Touch
    /// ID prompt (or fall back to the device passcode if Touch ID fails).
    ///
    /// Returns a [`SecretString`] so the password is zeroized on drop.
    pub fn load_password() -> Result<SecretString> {
        let service = CFString::new(SERVICE);
        let account = CFString::new(ACCOUNT);
        let reason = CFString::new(REASON);

        unsafe {
            let mut dict = CFMutableDictionary::new();

            dict.set(
                CFType::wrap_under_get_rule(kSecClass as *const _),
                CFType::wrap_under_get_rule(kSecClassGenericPassword as *const _),
            );
            dict.set(
                CFType::wrap_under_get_rule(kSecAttrService as *const _),
                service.as_CFType(),
            );
            dict.set(
                CFType::wrap_under_get_rule(kSecAttrAccount as *const _),
                account.as_CFType(),
            );
            dict.set(
                CFType::wrap_under_get_rule(kSecReturnData as *const _),
                CFBoolean::true_value().as_CFType(),
            );
            dict.set(
                CFType::wrap_under_get_rule(kSecMatchLimit as *const _),
                CFType::wrap_under_get_rule(kSecMatchLimitOne as *const _),
            );
            dict.set(
                CFType::wrap_under_get_rule(kSecUseOperationPrompt as *const _),
                reason.as_CFType(),
            );

            let mut result: core_foundation::base::CFTypeRef = ptr::null();
            let rc = SecItemCopyMatching(dict.as_concrete_TypeRef(), &mut result);

            if rc == ERR_SEC_ITEM_NOT_FOUND {
                bail!(
                    "No biometric credentials found in Keychain.\n\
                     Run `lk unlock --save-biometric` to save your vault password first."
                );
            } else if rc != ERR_SEC_SUCCESS {
                bail!(
                    "Keychain retrieval failed (code {}). \
                     Touch ID may be unavailable — use `lk unlock` with your password instead.",
                    rc
                );
            }

            let data = CFData::wrap_under_create_rule(result as *mut _);
            let bytes = data.bytes().to_vec();
            let pwd = String::from_utf8(bytes).map_err(|e| {
                // Recover and zeroize the raw bytes before surfacing the error
                // so the password material does not linger in memory.
                let mut b = e.into_bytes();
                b.zeroize();
                anyhow::anyhow!("Keychain returned invalid UTF-8 for vault password")
            })?;
            Ok(SecretString::new(pwd))
        }
    }

    /// Remove the Keychain item (e.g. when the vault password changes).
    pub fn delete_saved_password() -> Result<()> {
        let deleted = delete_item();
        if deleted {
            Ok(())
        } else {
            anyhow::bail!("No biometric credentials found in Keychain — nothing to delete.");
        }
    }

    // ── Internal helpers ──────────────────────────────────────────────────────

    fn delete_item() -> bool {
        let service = CFString::new(SERVICE);
        let account = CFString::new(ACCOUNT);
        unsafe {
            let mut dict = CFMutableDictionary::new();
            dict.set(
                CFType::wrap_under_get_rule(kSecClass as *const _),
                CFType::wrap_under_get_rule(kSecClassGenericPassword as *const _),
            );
            dict.set(
                CFType::wrap_under_get_rule(kSecAttrService as *const _),
                service.as_CFType(),
            );
            dict.set(
                CFType::wrap_under_get_rule(kSecAttrAccount as *const _),
                account.as_CFType(),
            );
            let rc = SecItemDelete(dict.as_concrete_TypeRef());
            rc == ERR_SEC_SUCCESS
        }
    }

    /// Create a `SecAccessControl` object that requires biometric (Touch ID /
    /// Face ID) with a device-passcode fallback.
    fn create_biometric_access_control() -> Result<*mut std::ffi::c_void> {
        unsafe {
            let mut error: core_foundation::base::CFTypeRef = ptr::null();
            let flags = SEC_ACCESS_CONTROL_BIOMETRY_ANY
                | SEC_ACCESS_CONTROL_OR
                | SEC_ACCESS_CONTROL_DEVICE_PASSCODE;
            let acl = SecAccessControlCreateWithFlags(
                kCFAllocatorDefault,
                kSecAttrAccessibleWhenUnlockedThisDeviceOnly as *const _,
                flags,
                &mut error,
            );
            if acl.is_null() {
                let msg = if error.is_null() {
                    "unknown error".to_string()
                } else {
                    format!("error ref {:p}", error)
                };
                bail!(
                    "Failed to create biometric access control: {}. \
                     Ensure Touch ID is configured in System Preferences.",
                    msg
                );
            }
            Ok(acl)
        }
    }
}

// ── Fallback for non-macOS platforms ──────────────────────────────────────────

#[cfg(not(target_os = "macos"))]
mod fallback_impl {
    use anyhow::{Result, bail};
    use secrecy::SecretString;

    pub fn save_password(_password: &str) -> Result<()> {
        bail!(
            "Biometric unlock (Touch ID / Face ID) is only supported on macOS.\n\
             Use `lk unlock` with your master password instead."
        )
    }

    pub fn load_password() -> Result<SecretString> {
        bail!(
            "Biometric unlock (Touch ID / Face ID) is only supported on macOS.\n\
             Use `lk unlock` with your master password instead."
        )
    }

    pub fn delete_saved_password() -> Result<()> {
        bail!("Biometric unlock (Touch ID / Face ID) is only supported on macOS.")
    }
}

// ── Public re-exports ─────────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
pub use macos_impl::{delete_saved_password, load_password, save_password};

#[cfg(not(target_os = "macos"))]
pub use fallback_impl::{delete_saved_password, load_password, save_password};
