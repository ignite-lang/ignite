#![allow(unused_mut, unused_variables, dead_code, unused_parens, unused_imports, unreachable_code, non_snake_case, bindings_with_variant_name, clippy::all)]

pub fn shape(xs: &Vec<i64>) -> i64 {
    return match xs {
        __m if (__m.len() == 0) => 0,
        __m if (__m.len() == 1) => 1,
        __m if (__m.len() >= 2) => 2,
        #[allow(unreachable_patterns)]
        _ => ::ignite_io::fail("match", "crates/ignite-rustgen/tests/golden/rest.ignite", 4, 10, "no arm matched"),
    };
}

pub fn tail(xs: &Vec<String>) -> Vec<String> {
    return match xs {
        __m if (__m.len() >= 1) => { let rest = __m[1..].to_vec(); rest.clone() },
        __m if (__m.len() == 0) => vec![],
        #[allow(unreachable_patterns)]
        _ => ::ignite_io::fail("match", "crates/ignite-rustgen/tests/golden/rest.ignite", 12, 10, "no arm matched"),
    };
}

pub fn ends(xs: &Vec<i64>) -> i64 {
    return match xs {
        __m if (__m.len() >= 2) => { let a = ((&__m[0])).clone(); let z = ((&__m[__m.len() - 1])).clone(); ::ignite_io::checked::add(a, z, "crates/ignite-rustgen/tests/golden/rest.ignite", 20, 19) },
        __m if (__m.len() == 1) => { let a = ((&__m[0])).clone(); a },
        __m if (__m.len() == 0) => 0,
        #[allow(unreachable_patterns)]
        _ => ::ignite_io::fail("match", "crates/ignite-rustgen/tests/golden/rest.ignite", 19, 10, "no arm matched"),
    };
}

pub fn after_command(bytes: &Vec<u8>) -> Vec<u8> {
    return match bytes {
        __m if (__m.len() >= 2 && { let __p11 = &__m[0]; (*__p11 == 255) }) => { let rest = __m[2..].to_vec(); rest.clone() },
        _ => bytes.clone(),
    };
}

pub fn status(request: &(Vec<String>, i64)) -> i64 {
    return match request {
        __m if { let (__p14, _) = __m; (__p14.len() >= 2) } => 400,
        (_, _) => 200,
        #[allow(unreachable_patterns)]
        _ => ::ignite_io::fail("match", "crates/ignite-rustgen/tests/golden/rest.ignite", 34, 10, "no arm matched"),
    };
}
