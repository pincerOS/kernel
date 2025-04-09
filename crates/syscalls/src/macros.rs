#[macro_export]
macro_rules! syscall {
    ($num:expr => $vis:vis fn $ident:ident ( $($arg:ident : $ty:ty),* $(,)? ) $( -> $ret:ty )?) => {
        core::arch::global_asm!(
            ".global {name}; {name}: svc #{num}; ret",
            name = sym $ident,
            num = const $num,
        );
        unsafe extern "C" {
            $vis fn $ident( $($arg: $ty,)* ) $(-> $ret)?;
        }
    };
}

#[macro_export]
macro_rules! def_enum {
    ($vis:vis $name:ident => $ty:ty {
        $($variant:ident => $val:expr),+
        $(,)?
    }) => {
        #[non_exhaustive]
        $vis struct $name;

        impl $name {
            $(
                pub const $variant: $ty = $val;
            )+

            pub const VARIANT_VALUES: &'static [$ty] = &[$(Self::$variant),+];
            pub const VARIANTS: &'static [&'static str] = &[
                $(stringify!($variant)),+
            ];
        }
    };
}

// very janky macro, not really worth it...
#[macro_export]
macro_rules! define_variants {
    ($variants:expr, $($variant:expr => $vis:vis fn $ident:ident ( $($arg:ident : $ty:ty),* $(,)? ) $( -> $ret:ty )?),* $(,)?) => {
        const _: () = {
            // Use a constant expression to enforce compile-time checking
            const fn check_all_variants_defined() {
                // Get the variants array
                const VARIANTS: &[&'static str] = $variants;

                // Create a compile-time array to track which variants have been defined
                let mut defined = [false; VARIANTS.len()];

                // Mark each variant as defined
                $(
                    let mut found = false;
                    let current_variant: &[u8] = stringify!($variant).as_bytes();

                    let mut i = 0;
                    while i < VARIANTS.len() {
                        // Convert variant to string for comparison
                        let mut match_found;
                        let var_bytes = VARIANTS[i].as_bytes();

                        // Extract the portion after :: from the current variant
                        let mut variant_start = 0;
                        let mut j = current_variant.len();
                        // Find the start of the variant name (after ::)
                        while j > 0 {
                            j -= 1;
                            if current_variant[j] == b':' && j > 0 && current_variant[j-1] == b':' {
                                variant_start = j + 1;
                                break;
                            }
                        }

                        // Compare only the variant name part
                        if var_bytes.len() == current_variant.len() - variant_start {
                            let mut k = 0;
                            match_found = true;
                            while k < var_bytes.len() {
                                if var_bytes[k] != current_variant[variant_start + k] {
                                    match_found = false;
                                    break;
                                }
                                k += 1;
                            }

                            if match_found {
                                defined[i] = true;
                                found = true;
                                break;
                            }
                        }
                        i += 1;
                    }

                    // Ensure this variant exists in our array
                    assert!(found, concat!("Unknown variant specified: ", stringify!($variant)));
                )*

                // Check that all variants are defined
                let mut i = 0;
                while i < defined.len() {
                    assert!(defined[i], "Missing definition for a variant");
                    i += 1;
                }
            }

            // Call the check function
            check_all_variants_defined();
        };

        // Generate the actual function calls for each variant
        $(
            syscall!($variant => $vis fn $ident ( $($arg: $ty),* ) $(-> $ret)?);
        )*
    }
}

// this is even worse
#[macro_export]
macro_rules! register_variants {
    ($($variant:expr => $handler:expr),* $(,)?) => {
        const _: () = {
            // Use a constant expression to enforce compile-time checking
            const fn check_all_variants_defined() {
                // Get the variants array
                const VARIANTS: &[&'static str] = SysCall::VARIANTS;

                // Create a compile-time array to track which variants have been defined
                let mut defined = [false; VARIANTS.len()];

                // Mark each variant as defined
                $(
                    let mut found = false;
                    let current_variant: &[u8] = stringify!($variant).as_bytes();

                    let mut i = 0;
                    while i < VARIANTS.len() {
                        // Convert variant to string for comparison
                        let mut match_found;
                        let var_bytes = VARIANTS[i].as_bytes();

                        // Extract the portion after :: from the current variant
                        let mut variant_start = 0;
                        let mut j = current_variant.len();
                        // Find the start of the variant name (after ::)
                        while j > 0 {
                            j -= 1;
                            if current_variant[j] == b':' && j > 0 && current_variant[j-1] == b':' {
                                variant_start = j + 1;
                                break;
                            }
                        }

                        // Compare only the variant name part
                        if var_bytes.len() == current_variant.len() - variant_start {
                            let mut k = 0;
                            match_found = true;
                            while k < var_bytes.len() {
                                if var_bytes[k] != current_variant[variant_start + k] {
                                    match_found = false;
                                    break;
                                }
                                k += 1;
                            }

                            if match_found {
                                defined[i] = true;
                                found = true;
                                break;
                            }
                        }
                        i += 1;
                    }

                    // Ensure this variant exists in our array
                    assert!(found, concat!("Unknown variant specified: ", stringify!($variant)));
                )*

                // Check that all variants are defined
                let mut i = 0;
                while i < defined.len() {
                    assert!(defined[i], "Missing definition for a variant");
                    i += 1;
                }
            }

            // Call the check function
            check_all_variants_defined();
        };

        $(
            register_syscall_handler($variant as usize, $handler);
        )*
    }
}
