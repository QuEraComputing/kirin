/// Safe division that returns `None` on division by zero instead of panicking.
pub trait CheckedDiv: Sized {
    fn checked_div(self, rhs: Self) -> Option<Self>;
}

/// Safe remainder that returns `None` on division by zero instead of panicking.
pub trait CheckedRem: Sized {
    fn checked_rem(self, rhs: Self) -> Option<Self>;
}

macro_rules! impl_checked_int {
    ($($t:ty),*) => {
        $(
            impl CheckedDiv for $t {
                fn checked_div(self, rhs: Self) -> Option<Self> {
                    <$t>::checked_div(self, rhs)
                }
            }
            impl CheckedRem for $t {
                fn checked_rem(self, rhs: Self) -> Option<Self> {
                    <$t>::checked_rem(self, rhs)
                }
            }
        )*
    };
}

impl_checked_int!(i8, i16, i32, i64, i128, u8, u16, u32, u64, u128);

macro_rules! impl_checked_float {
    ($($t:ty),*) => {
        $(
            impl CheckedDiv for $t {
                fn checked_div(self, rhs: Self) -> Option<Self> {
                    // Float division by zero produces infinity/NaN, not a panic.
                    Some(self / rhs)
                }
            }
            impl CheckedRem for $t {
                fn checked_rem(self, rhs: Self) -> Option<Self> {
                    // Float remainder by zero produces NaN, not a panic.
                    Some(self % rhs)
                }
            }
        )*
    };
}

impl_checked_float!(f32, f64);
