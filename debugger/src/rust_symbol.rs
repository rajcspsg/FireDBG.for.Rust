//! Rust symbol strings from LLDB differ by platform: Linux often retains a trailing `::h<hex>`
//! disambiguator on demangled names. Strip it so event streams match other hosts and tests.

/// Normalize a demangled Rust function name from LLDB.
///
/// Example: `call_chain::head::h4766c160f69ecd87` → `call_chain::head`.
pub fn normalize_lldb_rust_function_name(name: &str) -> String {
    strip_rust_symbol_hash_suffix(name).to_owned()
}

fn strip_rust_symbol_hash_suffix(name: &str) -> &str {
    let Some(pos) = name.rfind("::h") else {
        return name;
    };
    let rest = &name[pos + 3..];
    // Require at least 8 hex digits to avoid stripping accidental `::h` inside paths.
    if rest.len() >= 8 && rest.bytes().all(|b| b.is_ascii_hexdigit()) {
        &name[..pos]
    } else {
        name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_trailing_disambiguator() {
        assert_eq!(
            normalize_lldb_rust_function_name("call_chain::head::h4766c160f69ecd87"),
            "call_chain::head"
        );
    }

    #[test]
    fn leaves_short_or_non_hex_suffix() {
        assert_eq!(
            normalize_lldb_rust_function_name("call_chain::head::h12ab"),
            "call_chain::head::h12ab"
        );
        assert_eq!(
            normalize_lldb_rust_function_name("call_chain::hash::module"),
            "call_chain::hash::module"
        );
    }
}
