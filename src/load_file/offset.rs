use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Rem, RemAssign, Sub, SubAssign};

/// An absolute (non-negative) offset from the beginning of a core.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Offset {
    value: i32,
    core_size: i32,
}

fn offset_value(value: i32, core_size: i32) -> i32 {
    value.rem_euclid(core_size)
}

impl Offset {
    /// Create a new Offset. The value will be adjusted to be within bounds of the core.
    ///
    /// # Panics
    /// If `core_size` is invalid. Both 0 and `i32::MAX` are disallowed.
    #[must_use]
    pub fn new(value: i32, core_size: i32) -> Self {
        Self {
            value: offset_value(value, core_size),
            core_size,
        }
    }

    /// Get the value of the offset. This will always be less than the core size.
    #[must_use]
    pub fn value(&self) -> i32 {
        self.value
    }

    /// Verify another offset has the same core size. Panics otherwise
    fn check_core_size(self, other: Self) {
        assert_eq!(
            self.core_size, other.core_size,
            "attempt to add mismatching core sizes: {} != {}",
            self.core_size, other.core_size,
        );
    }
}

impl std::fmt::Display for Offset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.value.fmt(f)
    }
}

/// Implement a `std::ops` operation for `Offset`.
macro_rules! impl_offset_op {
    (
        $op_trait:ident :: $op:ident ,
        $assign_trait:ident :: $assign:ident $(,)?
    ) => {
        impl $op_trait for Offset {
            type Output = Self;

            // Note $-expansion doesn't happen in doc comments. If needed there
            // is a workaround in https://github.com/rust-lang/rust/issues/52607

            /// Panics if the  right-hand side has a different `core_size`
            /// than the left-hand side.
            fn $op(self, rhs: Self) -> Self {
                self.check_core_size(rhs);
                Self::new((self.value).$op(rhs.value), self.core_size)
            }
        }

        impl $assign_trait for Offset {
            fn $assign(&mut self, rhs: Self) {
                // check_core_size is called by $op_trait::$op
                *self = self.$op(rhs)
            }
        }
    };
}

impl_offset_op! { Add::add, AddAssign::add_assign }
impl_offset_op! { Sub::sub, SubAssign::sub_assign }
impl_offset_op! { Mul::mul, MulAssign::mul_assign }
impl_offset_op! { Div::div, DivAssign::div_assign }
impl_offset_op! { Rem::rem, RemAssign::rem_assign }

/// Implement a `std::ops` operation for `Offset` and another type
macro_rules! impl_op {
    (
        $rhs:ty,
        $op_trait:ident :: $op:ident ,
        $assign_trait:ident :: $assign:ident $(,)?
    ) => {
        impl $op_trait<$rhs> for Offset {
            type Output = Self;

            fn $op(self, rhs: $rhs) -> Self::Output {
                self.$op(Self::new(rhs as i32, self.core_size))
            }
        }

        impl $assign_trait<$rhs> for Offset {
            fn $assign(&mut self, rhs: $rhs) {
                self.value = offset_value((self.$op(rhs)).value, self.core_size)
            }
        }
    };
}

impl_op! { i32, Add::add, AddAssign::add_assign }
impl_op! { i32, Div::div, DivAssign::div_assign }
impl_op! { i32, Mul::mul, MulAssign::mul_assign }
impl_op! { i32, Rem::rem, RemAssign::rem_assign }
impl_op! { i32, Sub::sub, SubAssign::sub_assign }

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn create_offset() {
        let offset = Offset::new(1234, 12);
        assert_eq!(offset.value(), 10);
    }

    #[test]
    fn add_offset() {
        let mut offset = Offset::new(0, 12);

        assert_eq!(offset + 17_i32, Offset::new(5, 12));
        assert_eq!(offset + -17_i32, Offset::new(7, 12));
        assert_eq!(offset + Offset::new(17, 12), Offset::new(5, 12));
        assert_eq!(offset + Offset::new(-17, 12), Offset::new(7, 12));
        assert_eq!(offset + 17_i32, Offset::new(5, 12));

        offset += 17_i32;
        assert_eq!(offset, Offset::new(5, 12));
        offset = Offset::new(0, 12);

        offset += -17_i32;
        assert_eq!(offset, Offset::new(7, 12));
        offset = Offset::new(0, 12);

        offset += Offset::new(17, 12);
        assert_eq!(offset, Offset::new(5, 12));
        offset = Offset::new(0, 12);

        offset += Offset::new(-17, 12);
        assert_eq!(offset, Offset::new(7, 12));
        offset = Offset::new(0, 12);

        offset += 17_i32;
        assert_eq!(offset, Offset::new(5, 12));
    }

    #[test]
    fn sub_offset() {
        let mut offset = Offset::new(0, 12);

        assert_eq!(offset - 17_i32, Offset::new(7, 12));
        assert_eq!(offset - -17_i32, Offset::new(5, 12));
        assert_eq!(offset - Offset::new(17, 12), Offset::new(7, 12));
        assert_eq!(offset - Offset::new(-17, 12), Offset::new(5, 12));
        assert_eq!(offset - 17_i32, Offset::new(7, 12));

        offset -= 17_i32;
        assert_eq!(offset, Offset::new(7, 12));

        offset = Offset::new(0, 12);
        offset -= -17_i32;
        assert_eq!(offset, Offset::new(5, 12));

        offset = Offset::new(0, 12);
        offset -= Offset::new(17, 12);
        assert_eq!(offset, Offset::new(7, 12));

        offset = Offset::new(0, 12);
        offset -= Offset::new(-17, 12);
        assert_eq!(offset, Offset::new(5, 12));

        offset = Offset::new(0, 12);
        offset -= 17_i32;
        assert_eq!(offset, Offset::new(7, 12));
    }

    #[test]
    fn mul_offset() {
        let mut offset = Offset::new(2, 12);

        assert_eq!(offset * 5_i32, Offset::new(10, 12));
        assert_eq!(offset * -5_i32, Offset::new(2, 12));
        assert_eq!(offset * Offset::new(5, 12), Offset::new(10, 12));
        assert_eq!(offset * Offset::new(-5, 12), Offset::new(2, 12));
        assert_eq!(offset * 5_i32, Offset::new(10, 12));

        offset *= 5_i32;
        assert_eq!(offset, Offset::new(10, 12));

        offset = Offset::new(2, 12);
        offset *= -5_i32;
        assert_eq!(offset, Offset::new(2, 12));

        offset = Offset::new(2, 12);
        offset *= Offset::new(5, 12);
        assert_eq!(offset, Offset::new(10, 12));

        offset = Offset::new(2, 12);
        offset *= Offset::new(-5, 12);
        assert_eq!(offset, Offset::new(2, 12));

        offset = Offset::new(2, 12);
        offset *= 5_i32;
        assert_eq!(offset, Offset::new(10, 12));
    }

    #[test]
    fn div_offset() {
        let mut offset = Offset::new(10, 12);

        assert_eq!(offset / 5_i32, Offset::new(2, 12));
        assert_eq!(offset / -5_i32, Offset::new(1, 12));
        assert_eq!(offset / Offset::new(5, 12), Offset::new(2, 12));
        assert_eq!(offset / Offset::new(-5, 12), Offset::new(1, 12));
        assert_eq!(offset / 5_i32, Offset::new(2, 12));

        offset /= 5_i32;
        assert_eq!(offset, Offset::new(2, 12));

        offset = Offset::new(10, 12);
        offset /= -5_i32;
        assert_eq!(offset, Offset::new(1, 12));

        offset = Offset::new(10, 12);
        offset /= Offset::new(5, 12);
        assert_eq!(offset, Offset::new(2, 12));

        offset = Offset::new(10, 12);
        offset /= Offset::new(-5, 12);
        assert_eq!(offset, Offset::new(1, 12));

        offset = Offset::new(10, 12);
        offset /= 5_i32;
        assert_eq!(offset, Offset::new(2, 12));
    }

    #[test]
    fn rem_offset() {
        let mut offset = Offset::new(8, 12);

        assert_eq!(offset % 5_i32, Offset::new(3, 12));
        assert_eq!(offset % -5_i32, Offset::new(1, 12));
        assert_eq!(offset % Offset::new(5, 12), Offset::new(3, 12));
        assert_eq!(offset % Offset::new(-5, 12), Offset::new(1, 12));
        assert_eq!(offset % 5_i32, Offset::new(3, 12));

        offset %= 5_i32;
        assert_eq!(offset, Offset::new(3, 12));

        offset = Offset::new(8, 12);
        offset %= -5_i32;
        assert_eq!(offset, Offset::new(1, 12));

        offset = Offset::new(8, 12);
        offset %= Offset::new(5, 12);
        assert_eq!(offset, Offset::new(3, 12));

        offset = Offset::new(8, 12);
        offset %= Offset::new(-5, 12);
        assert_eq!(offset, Offset::new(1, 12));

        offset = Offset::new(8, 12);
        offset %= 5_i32;
        assert_eq!(offset, Offset::new(3, 12));
    }
}
