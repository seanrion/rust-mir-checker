use rug::Integer;
use std::cmp::Ordering;
use std::convert::TryFrom;
use std::fmt;
use std::ops::{Add, BitAnd, BitOr, BitXor, Div, Mul, Shl, Shr, Sub};

/// Either `∞`, `-∞`, or an arbitrary precision integer
#[derive(Clone, Eq, PartialEq)]
pub enum Bound {
    Infinity,          // Positive infinity
    Int(Integer), // Arbitrary precision integer
    NegativeInfinity,         // Negative infinity
}

use Bound::*;

impl Bound {
    pub fn is_finite(&self) -> bool {
        matches!(self, Int(_))
    }
}

impl fmt::Debug for Bound {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Infinity => String::from("∞"),
            NegativeInfinity => String::from("-∞"),
            Int(n) => n.to_string(),
        };
        write!(f, "{}", value)
    }
}

impl Ord for Bound {
    fn cmp(&self, other: &Self) -> Ordering {
        if self == other {
            Ordering::Equal
        } else {
            match (self, other) {
                (Infinity, _) | (_, NegativeInfinity) => Ordering::Greater,
                (NegativeInfinity, _) | (_, Infinity) => Ordering::Less,
                (Int(a), Int(b)) => a.cmp(b),
            }
        }
    }
}

impl PartialOrd for Bound {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl From<Integer> for Bound {
    fn from(n: Integer) -> Self {
        Bound::Int(n)
    }
}

impl From<i128> for Bound {
    fn from(n: i128) -> Self {
        Bound::Int(Integer::from(n))
    }
}

impl From<u128> for Bound {
    fn from(n: u128) -> Self {
        Bound::Int(Integer::from(n))
    }
}

impl Add for Bound {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        match (self, other) {
            // FIXME Here, `INF + NINF` = INF, does it matter?
            (Infinity, _) | (_, Infinity) => Self::Infinity,
            (NegativeInfinity, _) | (_, NegativeInfinity) => Self::NegativeInfinity,
            (Int(a), Int(b)) => Self::Int(a + b),
        }
    }
}

impl Sub for Bound {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        match (self, other) {
            // FIXME Here `INF - INF = INF`, `NINF - NINF = INF`, does it matter?
            (Infinity, _) | (_, NegativeInfinity) => Self::Infinity,
            (NegativeInfinity, _) | (_, Infinity) => Self::NegativeInfinity,
            (Int(a), Int(b)) => Self::Int(a - b),
        }
    }
}

impl Mul for Bound {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self {
        enum Sign {
            Positive,
            Negative,
            Zero,
        }

        use Sign::*;

        let sign_lhs = match self {
            Infinity => Positive,
            NegativeInfinity => Negative,
            Int(ref n) if *n > 0 => Positive,
            Int(ref n) if *n < 0 => Negative,
            Int(_) => Zero,
        };
        let sign_rhs = match rhs {
            Infinity => Positive,
            NegativeInfinity => Negative,
            Int(ref n) if *n > 0 => Positive,
            Int(ref n) if *n < 0 => Negative,
            Int(_) => Zero,
        };
        let sign = match (sign_lhs, sign_rhs) {
            (Zero, _) | (_, Zero) => Zero,
            (Positive, Positive) | (Negative, Negative) => Positive,
            (Positive, Negative) | (Negative, Positive) => Negative,
        };
        match (self, rhs) {
            (Infinity, _) | (_, Infinity) | (NegativeInfinity, _) | (_, NegativeInfinity) => match sign {
                Positive => Infinity,
                Negative => NegativeInfinity,
                Zero => Int(Integer::from(0)),
            },
            (Int(a), Int(b)) => Self::Int(a * b),
        }
    }
}

impl Div for Bound {
    type Output = Self;

    fn div(self, rhs: Self) -> Self {
        enum Sign {
            Positive,
            Negative,
            Zero,
        }

        use Sign::*;

        let sign_lhs = match self {
            Infinity => Positive,
            NegativeInfinity => Negative,
            Int(ref n) if *n > 0 => Positive,
            Int(ref n) if *n < 0 => Negative,
            Int(_) => Zero,
        };
        let sign_rhs = match rhs {
            Infinity => Positive,
            NegativeInfinity => Negative,
            Int(ref n) if *n > 0 => Positive,
            Int(ref n) if *n < 0 => Negative,
            Int(_) => Zero,
        };
        let sign = match (sign_lhs, sign_rhs) {
            (Zero, _) => Zero,
            (Positive, Positive) | (Negative, Negative) => Positive,
            (Positive, Negative) | (Negative, Positive) => Negative,
            (_, Zero) => panic!("Division by zero"),
        };
        match (self, rhs) {
            (Infinity, _) | (NegativeInfinity, _) => match sign {
                Positive => Infinity,
                Negative => NegativeInfinity,
                Zero => Int(Integer::from(0)),
            },
            (_, Infinity) | (_, NegativeInfinity) => Int(Integer::from(0)),
            (Int(a), Int(b)) => Self::Int(a / b),
        }
    }
}

/// Abstract value that represents an interval
/// When `low` <= `high`, it is a normal interval `[low, high]`
/// When `low` == `NINF` && `high` == `INF`, it is `[-∞, ∞]`
/// When `low` == `INF` && `high` == `NINF`, it is `⊥`
#[derive(Clone, PartialEq)]
pub struct Interval {
    pub high: Bound,
    pub low: Bound,
}

impl fmt::Debug for Interval {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_bottom() {
            write!(f, "⊥")
        } else {
            write!(f, "[{:?}, {:?}]", self.low, self.high)
        }
    }
}

impl Interval {
    const INF: Bound = Bound::Infinity;
    const NINF: Bound = Bound::NegativeInfinity;

    pub fn new(low: Bound, high: Bound) -> Self {
        Interval { high, low }
    }

    pub fn top() -> Self {
        Interval {
            high: Self::INF,
            low: Self::NINF,
        }
    }

    pub fn bottom() -> Self {
        Interval {
            high: Self::NINF,
            low: Self::INF,
        }
    }

    pub fn is_top(&self) -> bool {
        self.high == Self::INF && self.low == Self::NINF
    }

    pub fn is_bottom(&self) -> bool {
        self.high < self.low
    }

    pub fn less_than(&self, other: &Interval) -> Option<bool> {
        if self.is_bottom() || self.is_top() || other.is_bottom() || other.is_top() {
            None
        } else if self.high < other.low {
            Some(true)
        } else if other.high <= self.low {
            Some(false)
        } else {
            None
        }
    }

    pub fn less_equal(&self, other: &Interval) -> Option<bool> {
        if self.is_bottom() || self.is_top() || other.is_bottom() || other.is_top() {
            None
        } else if self.high <= other.low {
            Some(true)
        } else if other.high < self.low {
            Some(false)
        } else {
            None
        }
    }

    pub fn greater_equal(&self, other: &Interval) -> Option<bool> {
        if self.is_bottom() || self.is_top() || other.is_bottom() || other.is_top() {
            None
        } else if self.low >= other.high {
            Some(true)
        } else if other.low > self.high {
            Some(false)
        } else {
            None
        }
    }

    pub fn greater_than(&self, other: &Interval) -> Option<bool> {
        if self.is_bottom() || self.is_top() || other.is_bottom() || other.is_top() {
            None
        } else if self.low > other.high {
            Some(true)
        } else if other.low >= self.high {
            Some(false)
        } else {
            None
        }
    }

    pub fn equal_to(&self, other: &Interval) -> Option<bool> {
        if let (Ok(v1), Ok(v2)) = (Integer::try_from(self), Integer::try_from(other)) {
            if v1 == v2 {
                return Some(true);
            } else {
                return Some(false);
            }
        }
        None
    }

    pub fn not_equal_to(&self, other: &Interval) -> Option<bool> {
        if let Some(bool_value) = self.equal_to(other) {
            return Some(!bool_value);
        }
        None
    }

    fn is_zero(&self) -> bool {
        self.low == Bound::Int(Integer::from(0)) && self.high == Bound::Int(Integer::from(0))
    }

    fn all_ones(&self) -> bool {
        self.low == Bound::Int(Integer::from(-1)) && self.high == Bound::Int(Integer::from(-1))
    }
}

impl TryFrom<Interval> for Integer {
    type Error = &'static str;
    fn try_from(value: Interval) -> Result<Self, Self::Error> {
        if let (Bound::Int(high), Bound::Int(low)) = (value.high, value.low) {
            if high == low {
                return Ok(high);
            }
        }
        Err("interval is not a integer")
    }
}

impl TryFrom<&Interval> for Integer {
    type Error = &'static str;
    fn try_from(value: &Interval) -> Result<Self, Self::Error> {
        if let (Bound::Int(high), Bound::Int(low)) = (value.high.clone(), value.low.clone()) {
            if high == low {
                return Ok(high);
            }
        }
        Err("interval is not a integer")
    }
}

impl TryFrom<Interval> for bool {
    type Error = &'static str;
    fn try_from(value: Interval) -> Result<Self, Self::Error> {
        if value.high == Bound::Int(Integer::from(1)) && value.low == Bound::Int(Integer::from(1)) {
            Ok(true)
        } else if value.high == Bound::Int(Integer::from(0))
            && value.low == Bound::Int(Integer::from(0))
        {
            Ok(false)
        } else {
            Err("interval is not a bool")
        }
    }
}

impl From<bool> for Interval {
    fn from(b: bool) -> Self {
        let v;
        if b {
            v = Bound::Int(Integer::from(1));
        } else {
            v = Bound::Int(Integer::from(0));
        }
        Interval {
            high: v.clone(),
            low: v,
        }
    }
}

impl Add for Interval {
    type Output = Interval;

    fn add(self, other: Interval) -> Interval {
        let low = self.low + other.low;
        let high = self.high + other.high;
        Interval::new(low, high)
    }
}

impl Sub for Interval {
    type Output = Interval;

    fn sub(self, other: Interval) -> Interval {
        let low = self.low - other.high;
        let high = self.high - other.low;
        Interval::new(low, high)
    }
}

impl Mul for Interval {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self {
        let a = self.low.clone() * rhs.low.clone();
        let b = self.low.clone() * rhs.high.clone();
        let c = self.high.clone() * rhs.low.clone();
        let d = self.high * rhs.high;
        let low = [a.clone(), b.clone(), c.clone(), d.clone()]
            .iter()
            .min()
            .unwrap()
            .clone();
        let high = [a, b, c, d].iter().max().unwrap().clone();
        Interval::new(low, high)
    }
}

impl Div for Interval {
    type Output = Self;

    fn div(self, rhs: Self) -> Self {
        let a = self.low.clone() / rhs.low.clone();
        let b = self.low.clone() / rhs.high.clone();
        let c = self.high.clone() / rhs.low.clone();
        let d = self.high / rhs.high;
        let low = [a.clone(), b.clone(), c.clone(), d.clone()]
            .iter()
            .min()
            .unwrap()
            .clone();
        let high = [a, b, c, d].iter().max().unwrap().clone();
        Interval::new(low, high)
    }
}

impl BitAnd for Interval {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        if self.is_bottom() || rhs.is_bottom() {
            Self::bottom()
        } else if self.is_top() || rhs.is_top() {
            Self::top()
        } else if self.is_zero() || rhs.is_zero() {
            Self::new(Bound::from(0u128), Bound::from(0u128))
        } else if self.all_ones() {
            rhs
        } else if rhs.all_ones() {
            self
        } else if let (Ok(lval), Ok(rval)) = (Integer::try_from(self), Integer::try_from(rhs)) {
            let and_val = lval & rval;
            Self::new(Bound::from(and_val.clone()), Bound::from(and_val))
        } else {
            Self::top()
        }
    }
}

impl BitOr for Interval {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        if self.is_bottom() || rhs.is_bottom() {
            Self::bottom()
        } else if self.is_top() || rhs.is_top() {
            Self::top()
        } else if self.all_ones() || rhs.all_ones() {
            Self::new(Bound::from(-1i128), Bound::from(-1i128))
        } else if self.is_zero() {
            rhs
        } else if rhs.is_zero() {
            self
        } else if let (Ok(lval), Ok(rval)) = (Integer::try_from(self), Integer::try_from(rhs)) {
            let or_val = lval | rval;
            Self::new(Bound::from(or_val.clone()), Bound::from(or_val))
        } else {
            Self::top()
        }
    }
}

impl BitXor for Interval {
    type Output = Self;

    fn bitxor(self, rhs: Self) -> Self::Output {
        if self.is_bottom() || rhs.is_bottom() {
            Self::bottom()
        } else if self.is_top() || rhs.is_top() {
            Self::top()
        } else if self.is_zero() {
            rhs
        } else if rhs.is_zero() {
            self
        } else if let (Ok(lval), Ok(rval)) = (Integer::try_from(self), Integer::try_from(rhs)) {
            let xor_val = lval ^ rval;
            Self::new(Bound::from(xor_val.clone()), Bound::from(xor_val))
        } else {
            Self::top()
        }
    }
}

impl Shl for Interval {
    type Output = Self;

    fn shl(self, rhs: Self) -> Self::Output {
        if self.is_bottom() || rhs.is_bottom() {
            Self::bottom()
        } else if self.is_top() || rhs.is_top() {
            Self::top()
        } else if let (Ok(lval), Ok(rval)) = (Integer::try_from(self), Integer::try_from(rhs)) {
            if let Some(u32val) = rval.to_u32() {
                let val = lval << u32val;
                Self::new(Bound::from(val.clone()), Bound::from(val))
            } else {
                Self::top()
            }
        } else {
            Self::top()
        }
    }
}

impl Shr for Interval {
    type Output = Self;

    fn shr(self, rhs: Self) -> Self::Output {
        if self.is_bottom() || rhs.is_bottom() {
            Self::bottom()
        } else if self.is_top() || rhs.is_top() {
            Self::top()
        } else if let (Ok(lval), Ok(rval)) = (Integer::try_from(self), Integer::try_from(rhs)) {
            if let Some(u32val) = rval.to_u32() {
                let val = lval >> u32val;
                Self::new(Bound::from(val.clone()), Bound::from(val))
            } else {
                Self::top()
            }
        } else {
            Self::top()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_integer_cmp() {
        let ninf = Bound::NegativeInfinity;
        let a = Bound::from(-1_i128);
        let b = Bound::from(0_i128);
        let c = Bound::from(1_i128);
        let inf = Bound::Infinity;
        assert!(ninf < a && a < b && b < c && c < inf);
    }
}
