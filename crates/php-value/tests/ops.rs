//! PHP 8's operators (src/ops.rs), against PHP 8.4.27's answers to the
//! same expressions (run in the oracle image).

use std::cmp::Ordering;

use php_value::{
    PhpArray, Value,
    ops::{add, bit_and, compare, concat, div, float_to_int, modulo, mul, neg, pow, sub, to_int},
};

fn s(text: &str) -> Value {
    Value::from(text)
}

fn error(result: Result<Value, php_value::Thrown>) -> String {
    let e = result
        .err()
        .map(|e| format!("{}: {}", e.class, e.message.to_string_lossy()));
    e.unwrap_or_default()
}

fn int(result: Result<Value, php_value::Thrown>) -> Option<i64> {
    match result {
        Ok(Value::Int(i)) => Some(i),
        _ => None,
    }
}

fn float(result: Result<Value, php_value::Thrown>) -> Option<f64> {
    match result {
        Ok(Value::Float(f)) => Some(f),
        _ => None,
    }
}

#[test]
fn arithmetic_converts_operands_as_php_8() {
    assert_eq!(
        error(add(&s(""), &Value::Int(1))),
        "TypeError: Unsupported operand types: string + int"
    );
    assert_eq!(
        error(mul(&s("abc"), &Value::Int(1))),
        "TypeError: Unsupported operand types: string * int"
    );
    assert_eq!(int(add(&s("12abc"), &Value::Int(1))), Some(13));
    assert_eq!(int(sub(&Value::Null, &Value::Int(1))), Some(-1));
    assert_eq!(float(add(&s("1.5"), &Value::Int(1))), Some(2.5));
    assert_eq!(int(add(&Value::Bool(true), &Value::Int(1))), Some(2));
    assert_eq!(
        error(add(
            &Value::Array(PhpArray::list([Value::Int(1)])),
            &Value::Int(1)
        )),
        "TypeError: Unsupported operand types: array + int"
    );
    assert_eq!(
        error(neg(&s("abc"))),
        "TypeError: Unsupported operand types: string * int"
    );
    assert_eq!(int(neg(&Value::Null)), Some(0));
    assert_eq!(
        float(add(&s("9223372036854775807"), &Value::Int(1))),
        Some(9.223_372_036_854_776e18)
    );
}

#[test]
fn array_plus_is_the_union() {
    let left = PhpArray::list([Value::Int(1)]);
    let right = PhpArray::list([Value::Int(2), Value::Int(3)]);
    let Ok(Value::Array(union)) = add(&Value::Array(left), &Value::Array(right)) else {
        panic!("not an array");
    };
    let values: Vec<i64> = union
        .values()
        .map(|v| if let Value::Int(i) = v { *i } else { 0 })
        .collect();
    assert_eq!(values, [1, 3]);
}

#[test]
fn division_modulo_and_power() {
    assert_eq!(float(div(&Value::Int(7), &Value::Int(2))), Some(3.5));
    assert_eq!(int(div(&Value::Int(6), &Value::Int(2))), Some(3));
    assert_eq!(
        float(div(&Value::Int(i64::MIN), &Value::Int(-1))),
        Some(9.223_372_036_854_776e18)
    );
    assert_eq!(
        error(div(&Value::Int(1), &Value::Int(0))),
        "DivisionByZeroError: Division by zero"
    );
    assert_eq!(
        error(modulo(&Value::Int(1), &Value::Int(0))),
        "DivisionByZeroError: Modulo by zero"
    );
    assert_eq!(int(modulo(&Value::Int(7), &Value::Int(-3))), Some(1));
    assert_eq!(int(modulo(&Value::Int(-7), &Value::Int(3))), Some(-1));
    assert_eq!(int(modulo(&Value::Float(1.5), &Value::Int(1))), Some(0));
    assert_eq!(int(modulo(&s("5"), &s("3"))), Some(2));
    assert_eq!(int(modulo(&Value::Int(i64::MIN), &Value::Int(-1))), Some(0));
    assert_eq!(int(pow(&Value::Int(2), &Value::Int(3))), Some(8));
    assert_eq!(
        float(pow(&Value::Int(2), &Value::Int(63))),
        Some(9.223_372_036_854_776e18)
    );
    assert_eq!(int(pow(&Value::Int(-2), &Value::Int(63))), Some(i64::MIN));
    assert_eq!(float(pow(&Value::Int(2), &Value::Int(-1))), Some(0.5));
    assert_eq!(int(pow(&s("2"), &s("3"))), Some(8));
}

#[test]
fn casts_and_bitwise() {
    assert_eq!(float_to_int(1e20), 7_766_279_631_452_241_920);
    assert_eq!(float_to_int(f64::NAN), 0);
    assert_eq!(to_int(&s("9999999999999999999")), i64::MAX);
    let Ok(Value::String(and)) = bit_and(&s("abc"), &s("ab")) else {
        panic!("not a string");
    };
    assert_eq!(and.as_bytes(), b"ab");
    assert_eq!(int(bit_and(&s("5"), &Value::Int(3))), Some(1));
    let Ok(Value::String(text)) = concat(&s("abc"), &Value::Array(PhpArray::new())) else {
        panic!("not a string");
    };
    assert_eq!(text.as_bytes(), b"abcArray");
}

#[test]
fn spaceship_follows_php_8() {
    let arr = |pairs: &[(&str, i64)]| {
        let mut a = PhpArray::new();
        for (k, v) in pairs {
            a.insert(*k, Value::Int(*v));
        }
        Value::Array(a)
    };
    let cases: [(Value, Value, Ordering); 13] = [
        (Value::Int(1), s("1a"), Ordering::Less),
        (s("10"), s("9"), Ordering::Greater),
        (s("10"), s("9a"), Ordering::Less),
        (Value::Null, s("a"), Ordering::Less),
        (Value::Null, Value::Int(0), Ordering::Equal),
        (Value::Null, Value::Int(-1), Ordering::Less),
        (
            Value::Array(PhpArray::new()),
            Value::Int(1),
            Ordering::Greater,
        ),
        (Value::Int(1), Value::Array(PhpArray::new()), Ordering::Less),
        (arr(&[("a", 1)]), arr(&[("b", 1)]), Ordering::Greater),
        (arr(&[("b", 1)]), arr(&[("a", 1)]), Ordering::Greater),
        (Value::Float(1.5), s("abc"), Ordering::Less),
        (Value::Bool(true), Value::Int(2), Ordering::Equal),
        (s("a"), Value::Null, Ordering::Greater),
    ];
    for (a, b, expected) in cases {
        assert_eq!(compare(&a, &b), expected, "{a:?} <=> {b:?}");
    }
}
