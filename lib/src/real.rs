use std::f64;
use std::num::Wrapping;
use std::convert::TryFrom;
use gml::symbol::Symbol;
use gml::{self, vm};

pub struct State {
    random_seed: Wrapping<i32>,
}

impl Default for State {
    fn default() -> State {
        State {
            random_seed: Wrapping(0),
        }
    }
}

#[gml::bind(Api)]
impl State {
    /// Emulate Delphi's LCG to advance the current random state.
    fn random_next(&mut self) -> Wrapping<i32> {
        self.random_seed = self.random_seed * Wrapping(0x8088405) + Wrapping(1);
        self.random_seed
    }

    /// Emulate Delphi's LCG to produce a random value in the range [0, x).
    ///
    /// This works by taking the top 32 bits of the 64-bit product of the seed and the max value.
    /// Because the seed is in the range [0, 2^32), it can be viewed as the fractional part of a
    /// 64-bit fixed-point number in the range [0, 1).
    fn random_u32(&mut self, x: u32) -> u32 {
        let Wrapping(seed) = self.random_next();
        (seed as u64 * x as u64 >> 32) as u32
    }

    /// Emulate Delphi's LCG to produce a random value in the range [0, 1).
    ///
    /// This works by dividing the seed's value, in the range [0, 2^32), by 2^32.
    fn random_f64(&mut self) -> f64 {
        let Wrapping(seed) = self.random_next();
        seed as f64 / 0x1_0000_0000u64 as f64
    }

    #[gml::function]
    pub fn random(&mut self, x: f64) -> f64 {
        self.random_f64() * x
    }

    #[gml::function]
    pub fn random_range(&mut self, x1: f64, x2: f64) -> f64 {
        x1 + self.random(x2 - x1)
    }

    #[gml::function]
    pub fn irandom(&mut self, x: u32) -> u32 {
        self.random_u32(x + 1)
    }

    #[gml::function]
    pub fn irandom_range(&mut self, x1: u32, x2: u32) -> u32 {
        x1 + self.irandom(x2 - x1 + 1)
    }

    #[gml::function]
    pub fn random_set_seed(&mut self, seed: i32) {
        self.random_seed = Wrapping(seed);
    }

    #[gml::function]
    pub fn random_get_seed(&mut self) -> i32 {
        let Wrapping(seed) = self.random_seed;
        seed
    }

    #[gml::function]
    pub fn randomize(&mut self) {
        // TODO: set the seed to the low bits of QueryPerformanceCounter
    }

    #[gml::function]
    pub fn choose(&mut self, vals: &[vm::Value]) -> vm::Value {
        vals[self.irandom(vals.len() as u32 - 1) as usize]
    }

    #[gml::function]
    pub fn abs(x: f64) -> f64 { f64::abs(x) }

    #[gml::function]
    pub fn sign(x: f64) -> f64 { f64::signum(x) }

    /// Round x to the nearest integer.
    ///
    /// The rounding actually occurs in i32::try_from(vm::Value), to ensure consistency with other
    /// instances of rounding in the language.
    #[gml::function]
    pub fn round(x: i32) -> i32 { x }

    #[gml::function]
    pub fn floor(x: f64) -> f64 { f64::floor(x) }

    #[gml::function]
    pub fn ceil(x: f64) -> f64 { f64::ceil(x) }

    #[gml::function]
    pub fn frac(x: f64) -> f64 { f64::fract(x) }

    #[gml::function]
    pub fn sqrt(x: f64) -> f64 { f64::sqrt(x) }

    #[gml::function]
    pub fn sqr(x: f64) -> f64 { x * x }

    #[gml::function]
    pub fn power(x: f64, n: f64) -> f64 { f64::powf(x, n) }

    #[gml::function]
    pub fn exp(x: f64) -> f64 { f64::exp(x) }

    #[gml::function]
    pub fn ln(x: f64) -> f64 { f64::ln(x) }

    #[gml::function]
    pub fn log2(x: f64) -> f64 { f64::log2(x) }

    #[gml::function]
    pub fn log10(x: f64) -> f64 { f64::log10(x) }

    #[gml::function]
    pub fn logn(n: f64, x: f64) -> f64 { f64::log(x, n) }

    #[gml::function]
    pub fn sin(x: f64) -> f64 { f64::sin(x) }

    #[gml::function]
    pub fn cos(x: f64) -> f64 { f64::cos(x) }

    #[gml::function]
    pub fn tan(x: f64) -> f64 { f64::tan(x) }

    #[gml::function]
    pub fn arcsin(x: f64) -> f64 { f64::asin(x) }

    #[gml::function]
    pub fn arccos(x: f64) -> f64 { f64::acos(x) }

    #[gml::function]
    pub fn arctan(x: f64) -> f64 { f64::atan(x) }

    #[gml::function]
    pub fn arctan2(y: f64, x: f64) -> f64 { f64::atan2(y, x) }

    #[gml::function]
    pub fn degtorad(x: f64) -> f64 { x / 180.0 * f64::consts::PI }

    #[gml::function]
    pub fn radtodeg(x: f64) -> f64 { x * 180.0 / f64::consts::PI }

    #[gml::function]
    pub fn min(vals: &[vm::Value]) -> vm::Value {
        let (mut min, rest) = match vals.split_first() {
            None => return vm::Value::from(0.0),
            Some((&first, rest)) => (first, rest),
        };
        for &val in rest {
            // GM treats `val` as the same type as `min` here, regardless of its actual type.
            match min.data() {
                vm::Data::Real(real) => if f64::try_from(val).unwrap_or_default() < real {
                    min = val;
                }
                vm::Data::String(string) => if Symbol::try_from(val).unwrap_or_default() < string {
                    min = val;
                }
                _ => {}
            }
        }
        min
    }

    #[gml::function]
    pub fn max(vals: &[vm::Value]) -> vm::Value {
        let (mut max, rest) = match vals.split_first() {
            None => return vm::Value::from(0.0),
            Some((&first, rest)) => (first, rest),
        };
        for &val in rest {
            // GM treats `val` as the same type as `max` here, regardless of its actual type.
            match max.data() {
                vm::Data::Real(real) => if f64::try_from(val).unwrap_or_default() > real {
                    max = val;
                }
                vm::Data::String(string) => if Symbol::try_from(val).unwrap_or_default() > string {
                    max = val;
                }
                _ => {}
            }
        }
        max
    }

    #[gml::function]
    pub fn mean(vals: &[vm::Value]) -> f64 {
        let sum: f64 = vals.iter().map(|&val| f64::try_from(val).unwrap_or_default()).sum();
        if vals.len() > 0 {
            sum / vals.len() as f64
        } else {
            0.0
        }
    }

    #[gml::function]
    pub fn median(vals: &[vm::Value]) -> f64 {
        // Because vm::Value is NaN-boxed, sorting shouldn't encounter NaNs.
        let mut vals: Vec<_> = vals.iter()
            .map(|&val| f64::try_from(val).unwrap_or_default())
            .collect();
        vals.sort_by(|a, b| f64::partial_cmp(a, b).unwrap());
        if vals.len() > 0 {
            vals[vals.len() / 2]
        } else {
            0.0
        }
    }

    #[gml::function]
    pub fn point_distance(x1: f64, y1: f64, x2: f64, y2: f64) -> f64 {
        let x = x2 - x1;
        let y = y2 - y1;
        f64::sqrt(x * x + y * y)
    }

    #[gml::function]
    pub fn point_direction(x1: f64, y1: f64, x2: f64, y2: f64) -> f64 {
        let x = x2 - x1;
        let y = y2 - y1;
        Self::radtodeg(Self::arctan2(y, x))
    }

    #[gml::function]
    pub fn lengthdir_x(len: f64, dir: f64) -> f64 {
        len * Self::cos(dir)
    }

    #[gml::function]
    pub fn lengthdir_y(len: f64, dir: f64) -> f64 {
        len * -Self::sin(dir)
    }

    #[gml::function]
    pub fn is_real(x: vm::Value) -> bool {
        x.data().ty() == vm::Type::Real
    }

    #[gml::function]
    pub fn is_string(x: vm::Value) -> bool {
        x.data().ty() == vm::Type::String
    }
}
