use std::{
    f64::consts::PI,
    ops::{Neg, Range},
    vec,
};

use hail::{
    ExecutorErrorKind, FunctionInfo, FunctionKind, Module, ModuleError, RangeIterator, ValueType,
};
use hail_macro::hail;

pub fn std() -> Result<Module, ModuleError> {
    let mut module = Module::default();

    // Custom handling of built-in types.
    // TODO: Remove these and stop handling built-ins special.
    module.typing.insert("i64".to_owned(), ValueType::Int);
    module.typing.insert("f64".to_owned(), ValueType::Float);
    module.typing.insert("bool".to_owned(), ValueType::Bool);
    module.typing.insert("String".to_owned(), ValueType::String);
    hail::define_type_wrappers::<i64>(&mut module, ValueType::Int)?;
    hail::define_type_wrappers::<f64>(&mut module, ValueType::Float)?;
    hail::define_type_wrappers::<bool>(&mut module, ValueType::Bool)?;
    hail::define_type_wrappers::<String>(&mut module, ValueType::String)?;

    // Ints
    hail_register_int_plus(&mut module)?;
    hail_register_int_minus(&mut module)?;
    hail_register_int_multiply(&mut module)?;
    hail_register_int_divide(&mut module)?;
    hail_register_int_remainder(&mut module)?;

    hail_register_int_plus_equal(&mut module)?;
    hail_register_int_minus_equal(&mut module)?;
    hail_register_int_multiply_equal(&mut module)?;
    hail_register_int_divide_equal(&mut module)?;
    hail_register_int_remainder_equal(&mut module)?;

    hail_register_int_greater(&mut module)?;
    hail_register_int_greater_equal(&mut module)?;
    hail_register_int_lesser(&mut module)?;
    hail_register_int_lesser_equal(&mut module)?;
    hail_register_int_equal(&mut module)?;
    hail_register_int_not_equal(&mut module)?;

    hail_register_int_negate(&mut module)?;

    // Bitwise operators for i64
    hail_register_int_and(&mut module)?;
    hail_register_int_or(&mut module)?;
    hail_register_int_xor(&mut module)?;
    hail_register_int_shift_left(&mut module)?;
    hail_register_int_shift_right(&mut module)?;

    hail_register_int_and_equal(&mut module)?;
    hail_register_int_or_equal(&mut module)?;
    hail_register_int_xor_equal(&mut module)?;

    hail_register_int_abs(&mut module)?;
    hail_register_int_pow(&mut module)?;
    hail_register_int_min(&mut module)?;
    hail_register_int_max(&mut module)?;

    hail_register_int_range_inclusive(&mut module)?;
    hail_register_int_range_exclusive(&mut module)?;
    hail_register_int_range_into_iter(&mut module)?;
    hail_register_int_iter_next(&mut module)?;

    hail_register_int_to_float(&mut module)?;

    // Floats
    module.register_global("PI", PI)?;

    hail_register_float_plus(&mut module)?;
    hail_register_float_minus(&mut module)?;
    hail_register_float_multiply(&mut module)?;
    hail_register_float_divide(&mut module)?;
    hail_register_float_remainder(&mut module)?;

    hail_register_float_plus_equal(&mut module)?;
    hail_register_float_minus_equal(&mut module)?;
    hail_register_float_multiply_equal(&mut module)?;
    hail_register_float_divide_equal(&mut module)?;
    hail_register_float_remainder_equal(&mut module)?;

    hail_register_float_greater(&mut module)?;
    hail_register_float_greater_equal(&mut module)?;
    hail_register_float_lesser(&mut module)?;
    hail_register_float_lesser_equal(&mut module)?;
    hail_register_float_equal(&mut module)?;
    hail_register_float_not_equal(&mut module)?;

    hail_register_float_negate(&mut module)?;
    hail_register_float_abs(&mut module)?;
    hail_register_float_pow(&mut module)?;
    hail_register_float_min(&mut module)?;
    hail_register_float_max(&mut module)?;
    hail_register_float_to_int(&mut module)?;
    hail_register_float_floor(&mut module)?;
    hail_register_float_ceiling(&mut module)?;
    hail_register_float_lerp(&mut module)?;
    hail_register_float_sin(&mut module)?;
    hail_register_float_cos(&mut module)?;
    hail_register_float_tan(&mut module)?;
    hail_register_float_sqrt(&mut module)?;

    // Bools
    hail_register_bool_equal(&mut module)?;
    hail_register_bool_not_equal(&mut module)?;
    hail_register_bool_invert(&mut module)?;

    // Strings
    hail_register_float_to_string_digits(&mut module)?;

    hail_register_int_to_string(&mut module)?;
    hail_register_float_to_string(&mut module)?;
    hail_register_bool_to_string(&mut module)?;

    hail_register_string_plus(&mut module)?;
    hail_register_string_plus_equal(&mut module)?;
    hail_register_string_plus_int(&mut module)?;
    hail_register_string_plus_float(&mut module)?;
    hail_register_string_plus_bool(&mut module)?;

    hail_register_string_equal(&mut module)?;
    hail_register_string_not_equal(&mut module)?;

    hail_register_string_len(&mut module)?;
    hail_register_string_is_empty(&mut module)?;
    hail_register_string_contains(&mut module)?;
    hail_register_string_repeat(&mut module)?;
    hail_register_string_substring(&mut module)?;
    hail_register_string_ends_with(&mut module)?;
    hail_register_string_find(&mut module)?;

    // Helper
    module.register_function(FunctionInfo {
        name: "todo".to_string(),
        param_types: vec![ValueType::String],
        param_names: vec!["message".to_string()],
        doc_comments: vec!["Mark that this point in the code is not complete yet.\nCauses a Todo execution error with the provided message.".to_string()],
        return_type: None,
        kind: FunctionKind::Free(|_executor, mut args| {
            let message = args
                .pop()
                .expect("Args should have todo message.")
                .into_any()
                .downcast::<String>()
                .expect("Expected string arg for todo");

            Err(ExecutorErrorKind::Todo(*message))
        }),
    })?;

    module.register_function(FunctionInfo {
        name: "fail".to_string(),
        param_types: vec![ValueType::String],
        param_names: vec!["message".to_string()],
        doc_comments: vec!["Cause a Fail execution error with the provided message.".to_string()],
        return_type: None,
        kind: FunctionKind::Free(|_executor, mut args| {
            let message = args
                .pop()
                .expect("Args should have fail message.")
                .into_any()
                .downcast::<String>()
                .expect("Expected string arg for fail");

            Err(ExecutorErrorKind::Fail(*message))
        }),
    })?;

    Ok(module)
}

//
// == Integers ==
//

#[hail(method, "+")]
fn int_plus(a: &i64, b: i64) -> i64 {
    *a + b
}

#[hail(method, "+=")]
fn int_plus_equal(a: &mut i64, b: i64) {
    *a += b
}

#[hail(method, "-")]
fn int_minus(a: &i64, b: i64) -> i64 {
    *a - b
}

#[hail(method, "-=")]
fn int_minus_equal(a: &mut i64, b: i64) {
    *a -= b
}

#[hail(method, "*")]
fn int_multiply(a: &i64, b: i64) -> i64 {
    *a * b
}

#[hail(method, "*=")]
fn int_multiply_equal(a: &mut i64, b: i64) {
    *a *= b
}

#[hail(method, "/")]
fn int_divide(a: &i64, b: i64) -> i64 {
    *a / b
}

#[hail(method, "/=")]
fn int_divide_equal(a: &mut i64, b: i64) {
    *a /= b
}

#[hail(method, "%")]
fn int_remainder(a: &i64, b: i64) -> i64 {
    *a % b
}

#[hail(method, "%=")]
fn int_remainder_equal(a: &mut i64, b: i64) {
    *a %= b
}

#[hail(method, ">")]
fn int_greater(a: &i64, b: i64) -> bool {
    *a > b
}

#[hail(method, ">=")]
fn int_greater_equal(a: &i64, b: i64) -> bool {
    *a >= b
}

#[hail(method, "<")]
fn int_lesser(a: &i64, b: i64) -> bool {
    *a < b
}

#[hail(method, "<=")]
fn int_lesser_equal(a: &i64, b: i64) -> bool {
    *a <= b
}

#[hail(method, "==")]
fn int_equal(a: &i64, b: i64) -> bool {
    *a == b
}

#[hail(method, "!=")]
fn int_not_equal(a: &i64, b: i64) -> bool {
    *a != b
}

#[hail(method, "-")]
fn int_negate(a: &i64) -> i64 {
    a.neg()
}

#[hail(method, "abs")]
fn int_abs(a: &i64) -> i64 {
    a.abs()
}

#[hail(method, "pow")]
fn int_pow(a: &i64, power: i64) -> i64 {
    a.pow(power as u32)
}

#[hail(method, "min")]
fn int_min(a: &i64, b: i64) -> i64 {
    (*a).min(b)
}

#[hail(method, "max")]
fn int_max(a: &i64, b: i64) -> i64 {
    (*a).max(b)
}

#[hail(method, "..")]
fn int_range_exclusive(a: &i64, b: i64) -> Range<i64> {
    Range { start: *a, end: b }
}

#[hail(method, "..=")]
fn int_range_inclusive(a: &i64, b: i64) -> Range<i64> {
    Range {
        start: *a,
        end: b + 1,
    }
}

#[hail(method, "into_iter")]
fn int_range_into_iter(range: &Range<i64>) -> RangeIterator<i64> {
    RangeIterator {
        current: range.start,
        end: range.end,
    }
}

/// Get the next value of a integer range iterator
#[hail(method, "next")]
fn int_iter_next(iter: &mut RangeIterator<i64>) -> Option<i64> {
    if iter.current < iter.end {
        let value = iter.current;
        iter.current += 1;
        Some(value)
    } else {
        None
    }
}

#[hail(method, "to_float")]
fn int_to_float(a: &i64) -> f64 {
    *a as f64
}

#[hail(method, "&")]
fn int_and(a: &i64, b: i64) -> i64 {
    a & b
}

#[hail(method, "|")]
fn int_or(a: &i64, b: i64) -> i64 {
    a | b
}

#[hail(method, "^")]
fn int_xor(a: &i64, b: i64) -> i64 {
    a ^ b
}

#[hail(method, "<<")]
fn int_shift_left(a: &i64, b: i64) -> i64 {
    a << b
}

#[hail(method, ">>")]
fn int_shift_right(a: &i64, b: i64) -> i64 {
    a >> b
}

#[hail(method, "&=")]
fn int_and_equal(a: &mut i64, b: i64) {
    *a &= b;
}

#[hail(method, "|=")]
fn int_or_equal(a: &mut i64, b: i64) {
    *a |= b;
}

#[hail(method, "^=")]
fn int_xor_equal(a: &mut i64, b: i64) {
    *a ^= b;
}

//
// == Floats ==
//

#[hail(method, "+")]
fn float_plus(a: &f64, b: f64) -> f64 {
    *a + b
}

#[hail(method, "+=")]
fn float_plus_equal(a: &mut f64, b: f64) {
    *a += b
}

#[hail(method, "-")]
fn float_minus(a: &f64, b: f64) -> f64 {
    *a - b
}

#[hail(method, "-=")]
fn float_minus_equal(a: &mut f64, b: f64) {
    *a -= b
}

#[hail(method, "*")]
fn float_multiply(a: &f64, b: f64) -> f64 {
    *a * b
}

#[hail(method, "*=")]
fn float_multiply_equal(a: &mut f64, b: f64) {
    *a *= b
}

#[hail(method, "/")]
fn float_divide(a: &f64, b: f64) -> f64 {
    *a / b
}

#[hail(method, "/=")]
fn float_divide_equal(a: &mut f64, b: f64) {
    *a /= b
}

#[hail(method, "%")]
fn float_remainder(a: &f64, b: f64) -> f64 {
    *a % b
}

#[hail(method, "%=")]
fn float_remainder_equal(a: &mut f64, b: f64) {
    *a %= b
}

#[hail(method, ">")]
fn float_greater(a: &f64, b: f64) -> bool {
    *a > b
}

#[hail(method, ">=")]
fn float_greater_equal(a: &f64, b: f64) -> bool {
    *a >= b
}

#[hail(method, "<")]
fn float_lesser(a: &f64, b: f64) -> bool {
    *a < b
}

#[hail(method, "<=")]
fn float_lesser_equal(a: &f64, b: f64) -> bool {
    *a <= b
}

#[hail(method, "==")]
fn float_equal(a: &f64, b: f64) -> bool {
    *a == b
}

#[hail(method, "!=")]
fn float_not_equal(a: &f64, b: f64) -> bool {
    *a != b
}

#[hail(method, "-")]
fn float_negate(a: &f64) -> f64 {
    a.neg()
}

#[hail(method, "abs")]
fn float_abs(a: &f64) -> f64 {
    a.abs()
}

#[hail(method, "pow")]
fn float_pow(a: &f64, power: f64) -> f64 {
    a.powf(power)
}

#[hail(method, "min")]
fn float_min(a: &f64, b: f64) -> f64 {
    a.min(b)
}

#[hail(method, "max")]
fn float_max(a: &f64, b: f64) -> f64 {
    a.max(b)
}

#[hail(method, "to_int")]
fn float_to_int(a: &f64) -> i64 {
    *a as i64
}

#[hail(method, "floor")]
fn float_floor(a: &f64) -> f64 {
    a.floor()
}

#[hail(method, "ceil")]
fn float_ceiling(a: &f64) -> f64 {
    a.ceil()
}

#[hail(method, "lerp")]
fn float_lerp(a: &f64, b: f64, t: f64) -> f64 {
    if t <= 0. {
        *a
    } else if t >= 1. {
        b
    } else {
        a + (b - a) * t
    }
}

#[hail(method, "sin")]
fn float_sin(a: &f64) -> f64 {
    a.sin()
}

#[hail(method, "cos")]
fn float_cos(a: &f64) -> f64 {
    a.cos()
}

#[hail(method, "tan")]
fn float_tan(a: &f64) -> f64 {
    a.tan()
}

#[hail(method, "sqrt")]
fn float_sqrt(a: &f64) -> f64 {
    a.sqrt()
}

//
// == Bool ==
//

#[hail(method, "==")]
fn bool_equal(a: &bool, b: bool) -> bool {
    *a == b
}

#[hail(method, "!=")]
fn bool_not_equal(a: &bool, b: bool) -> bool {
    *a != b
}

#[hail(method, "!")]
fn bool_invert(a: &bool) -> bool {
    !*a
}

//
// == Strings ==
//

#[hail(method, "to_string")]
fn int_to_string(a: &i64) -> String {
    a.to_string()
}

#[hail(method, "to_string")]
fn float_to_string(a: &f64) -> String {
    a.to_string()
}

/// Convert the f64 to a string with the given number of decimal ``digits``.
///
/// Returns the formatted string.
#[hail(method, "to_string")]
pub fn float_to_string_digits(val: &f64, digits: i64) -> String {
    let d = (digits.max(0)) as usize;
    if d == 0 {
        val.floor().to_string()
    } else {
        let multiplier = 10f64.powi(d as i32);
        let rounded = (val * multiplier).round() / multiplier;
        let s = format!("{:.}", rounded);
        if s.contains('.') {
            let parts: Vec<String> = s.split('.').map(|p| p.to_string()).collect();
            let mut decimals = parts[1].clone();
            while decimals.len() < d {
                decimals.push('0');
            }
            format!("{}.{}", parts[0], decimals)
        } else {
            let zeros = "0".repeat(d);
            format!("{}.{}", s, zeros)
        }
    }
}

#[hail(method, "to_string")]
fn bool_to_string(a: &bool) -> String {
    a.to_string()
}

#[hail(method, "+")]
fn string_plus(a: &String, b: String) -> String {
    a.to_owned() + b.as_str()
}

#[hail(method, "+=")]
fn string_plus_equal(a: &mut String, b: String) {
    *a += b.as_str()
}

#[hail(method, "+")]
fn string_plus_int(a: &String, b: i64) -> String {
    a.to_owned() + b.to_string().as_str()
}

#[hail(method, "+")]
fn string_plus_float(a: &String, b: f64) -> String {
    a.to_owned() + b.to_string().as_str()
}

#[hail(method, "+")]
fn string_plus_bool(a: &String, b: bool) -> String {
    a.to_owned() + b.to_string().as_str()
}

#[hail(method, "==")]
fn string_equal(a: &String, b: String) -> bool {
    *a == b
}

#[hail(method, "!=")]
fn string_not_equal(a: &String, b: String) -> bool {
    *a != b
}

#[hail(method, "len")]
fn string_len(a: &String) -> i64 {
    a.len() as i64
}

#[hail(method, "is_empty")]
fn string_is_empty(a: &String) -> bool {
    a.is_empty()
}

#[hail(method, "contains")]
fn string_contains(a: &String, b: String) -> bool {
    a.contains(&b)
}

#[hail(method, "repeat")]
fn string_repeat(a: &String, count: i64) -> String {
    a.repeat(count.max(0) as usize)
}

/// Get a part of the string with a ``start`` and exclusive ``end`` index.
///
/// Returns the partial string or an empty string for invalid args.
#[hail(method, "substring")]
fn string_substring(a: &String, start: i64, end: i64) -> String {
    let start = (start.max(0)) as usize;
    let end = (end.max(0) as usize).min(a.len());
    if start >= a.len() || end > a.len() || start >= end {
        return "".to_string();
    }

    a[start..end].to_string()
}

#[hail(method, "ends_with")]
fn string_ends_with(a: &String, b: String) -> bool {
    a.ends_with(&b)
}

/// Find the first instance of ``substring`` in this string.
///
/// Returns an Option<i64> with the index, or None if not found.
#[hail(method, "find")]
fn string_find(a: &String, substring: String) -> Option<i64> {
    a.find(&substring).map(|idx| idx as i64)
}

// Congrats, you're likely insane from scrolling through this.
//
