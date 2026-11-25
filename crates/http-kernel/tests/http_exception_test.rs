//! HttpExceptionTest.php and the subclass tests that extend it, at
//! symfony/http-kernel v8.1.11. Each upstream subclass test runs
//! `HttpExceptionTest`'s methods with its own `createException()` and adds
//! or overrides some; here each is a module of the same tests. Names and
//! assertions are PHPUnit's; `assertSame()` on header arrays is `===`.
//!
//! Upstream: symfony/http-kernel Tests/Exception/HttpExceptionTest.php Tests/Exception/AccessDeniedHttpExceptionTest.php Tests/Exception/BadRequestHttpExceptionTest.php Tests/Exception/ConflictHttpExceptionTest.php Tests/Exception/GoneHttpExceptionTest.php Tests/Exception/LengthRequiredHttpExceptionTest.php Tests/Exception/LockedHttpExceptionTest.php Tests/Exception/MethodNotAllowedHttpExceptionTest.php Tests/Exception/NotAcceptableHttpExceptionTest.php Tests/Exception/NotFoundHttpExceptionTest.php Tests/Exception/PreconditionFailedHttpExceptionTest.php Tests/Exception/PreconditionRequiredHttpExceptionTest.php Tests/Exception/ServiceUnavailableHttpExceptionTest.php Tests/Exception/TooManyRequestsHttpExceptionTest.php Tests/Exception/UnauthorizedHttpExceptionTest.php Tests/Exception/UnprocessableEntityHttpExceptionTest.php Tests/Exception/UnsupportedMediaTypeHttpExceptionTest.php

#![allow(non_snake_case)]

use php_value::{PhpArray, PhpString, Value, php_array};
use symfony_http_kernel::exception::{
    HttpArgs, HttpException, PhpClass, PhpException, Throwable, same,
};

/// `headerDataProvider()`.
fn header_data_provider() -> Vec<PhpArray> {
    vec![
        php_array!["X-Test" => "Test"],
        php_array!["X-Test" => 1_i64],
        PhpArray::list([
            Value::Array(php_array!["X-Test" => "Test"]),
            Value::Array(php_array!["X-Test-2" => "Test-2"]),
        ]),
    ]
}

/// `assertSame($expected, $actual)` for two header arrays.
fn assert_same_headers(expected: &PhpArray, actual: &PhpArray) {
    assert!(
        Value::Array(expected.clone()).identical(&Value::Array(actual.clone())),
        "{expected:?} !== {actual:?}"
    );
}

fn args(message: &str, previous: Option<Throwable>, code: i64, headers: PhpArray) -> HttpArgs {
    HttpArgs {
        message: PhpString::from(message),
        previous,
        code,
        headers,
    }
}

/// `provideStatusCode()`.
const STATUS_CODES: [i64; 16] = [
    400, 401, 403, 404, 406, 409, 410, 411, 412, 418, 423, 415, 422, 428, 429, 503,
];

/// `HttpExceptionTest`'s methods for one `createException()`.
macro_rules! http_exception_tests {
    ($module:ident, $create:expr) => {
        http_exception_tests!(@base $module, $create, {
            #[test]
            fn testHeadersDefault() {
                let exception: HttpException = ($create)(args("", None, 0, PhpArray::new()));
                assert_same_headers(&PhpArray::new(), exception.get_headers());
            }

            #[test]
            fn testHeadersSetter() {
                for headers in header_data_provider() {
                    let mut exception: HttpException =
                        ($create)(args("", None, 0, PhpArray::new()));
                    exception.set_headers(headers.clone());
                    assert_same_headers(&headers, exception.get_headers());
                }
            }
        });
    };
    (@base $module:ident, $create:expr, { $($extra:item)* }) => {
        mod $module {
            #[allow(unused_imports)]
            use super::*;

            $($extra)*

            #[test]
            fn testHeadersConstructor() {
                for headers in header_data_provider() {
                    let exception = HttpException::new(200, args("", None, 0, headers.clone()));
                    assert_same_headers(&headers, exception.get_headers());
                }
            }

            #[test]
            fn testThrowableIsAllowedForPrevious() {
                let previous = PhpException::new(PhpClass::Error, "Error of PHP 7+").throwable();
                let exception: HttpException =
                    ($create)(args("", Some(previous.clone()), 0, PhpArray::new()));
                assert!(same(&previous, exception.get_previous().unwrap()));
            }

            #[test]
            fn testFromStatusCode() {
                for status_code in STATUS_CODES {
                    let exception = HttpException::from_status_code(status_code, HttpArgs::default());
                    assert!(exception.is_a("Symfony\\Component\\HttpKernel\\Exception\\HttpException"));
                    assert_eq!(status_code, exception.get_status_code());
                }
            }
        }
    };
}

http_exception_tests!(http_exception_test, |a| HttpException::new(200, a));
http_exception_tests!(
    access_denied_http_exception_test,
    HttpException::access_denied
);
http_exception_tests!(bad_request_http_exception_test, HttpException::bad_request);
http_exception_tests!(conflict_http_exception_test, HttpException::conflict);
http_exception_tests!(gone_http_exception_test, HttpException::gone);
http_exception_tests!(
    length_required_http_exception_test,
    HttpException::length_required
);
http_exception_tests!(locked_http_exception_test, HttpException::locked);
http_exception_tests!(
    not_acceptable_http_exception_test,
    HttpException::not_acceptable
);
http_exception_tests!(not_found_http_exception_test, HttpException::not_found);
http_exception_tests!(
    precondition_failed_http_exception_test,
    HttpException::precondition_failed
);
http_exception_tests!(
    precondition_required_http_exception_test,
    HttpException::precondition_required
);
http_exception_tests!(
    unprocessable_entity_http_exception_test,
    HttpException::unprocessable_entity
);
http_exception_tests!(
    unsupported_media_type_http_exception_test,
    HttpException::unsupported_media_type
);

http_exception_tests!(@base method_not_allowed_http_exception_test,
    |a| HttpException::method_not_allowed(["get"], a),
    {
        #[test]
        fn testHeadersDefault() {
            let exception = HttpException::method_not_allowed(["GET", "PUT"], HttpArgs::default());
            assert_same_headers(&php_array!["Allow" => "GET, PUT"], exception.get_headers());
        }

        #[test]
        fn testWithHeaderConstruct() {
            let mut headers = php_array!["Cache-Control" => "public, s-maxage=1200"];

            let exception =
                HttpException::method_not_allowed(["get"], args("", None, 0, headers.clone()));

            headers.insert("Allow", Value::from("GET"));

            assert_same_headers(&headers, exception.get_headers());
        }

        #[test]
        fn testHeadersSetter() {
            for headers in header_data_provider() {
                let mut exception = HttpException::method_not_allowed(["GET"], HttpArgs::default());
                exception.set_headers(headers.clone());
                assert_same_headers(&headers, exception.get_headers());
            }
        }
    }
);

http_exception_tests!(@base service_unavailable_http_exception_test,
    |a| HttpException::service_unavailable(Value::Null, a),
    {
        #[test]
        fn testHeadersDefault() {
            let exception = HttpException::service_unavailable(Value::Null, HttpArgs::default());
            assert_same_headers(&PhpArray::new(), exception.get_headers());
        }

        #[test]
        fn testHeadersDefaultRetryAfter() {
            let exception = HttpException::service_unavailable(Value::Int(10), HttpArgs::default());
            assert_same_headers(&php_array!["Retry-After" => 10_i64], exception.get_headers());
        }

        #[test]
        fn testWithHeaderConstruct() {
            let mut headers = php_array!["Cache-Control" => "public, s-maxage=1337"];

            let exception = HttpException::service_unavailable(
                Value::Int(1337),
                args("", None, 0, headers.clone()),
            );

            headers.insert("Retry-After", Value::Int(1337));

            assert_same_headers(&headers, exception.get_headers());
        }

        #[test]
        fn testHeadersSetter() {
            for headers in header_data_provider() {
                let mut exception =
                    HttpException::service_unavailable(Value::Int(10), HttpArgs::default());
                exception.set_headers(headers.clone());
                assert_same_headers(&headers, exception.get_headers());
            }
        }
    }
);

http_exception_tests!(@base too_many_requests_http_exception_test,
    |a| HttpException::too_many_requests(Value::Null, a),
    {
        #[test]
        fn testHeadersDefault() {
            let exception = HttpException::too_many_requests(Value::Null, HttpArgs::default());
            assert_same_headers(&PhpArray::new(), exception.get_headers());
        }

        #[test]
        fn testHeadersDefaultRertyAfter() {
            let exception = HttpException::too_many_requests(Value::Int(10), HttpArgs::default());
            assert_same_headers(&php_array!["Retry-After" => 10_i64], exception.get_headers());
        }

        #[test]
        fn testWithHeaderConstruct() {
            let mut headers = php_array!["Cache-Control" => "public, s-maxage=69"];

            let exception = HttpException::too_many_requests(
                Value::Int(69),
                args("", None, 0, headers.clone()),
            );

            headers.insert("Retry-After", Value::Int(69));

            assert_same_headers(&headers, exception.get_headers());
        }

        #[test]
        fn testHeadersSetter() {
            for headers in header_data_provider() {
                let mut exception =
                    HttpException::too_many_requests(Value::Int(10), HttpArgs::default());
                exception.set_headers(headers.clone());
                assert_same_headers(&headers, exception.get_headers());
            }
        }
    }
);

http_exception_tests!(@base unauthorized_http_exception_test,
    |a| HttpException::unauthorized("Challenge", a),
    {
        #[test]
        fn testHeadersDefault() {
            let exception = HttpException::unauthorized("Challenge", HttpArgs::default());
            assert_same_headers(
                &php_array!["WWW-Authenticate" => "Challenge"],
                exception.get_headers(),
            );
        }

        #[test]
        fn testWithHeaderConstruct() {
            let mut headers = php_array!["Cache-Control" => "public, s-maxage=1200"];

            let exception =
                HttpException::unauthorized("Challenge", args("", None, 0, headers.clone()));

            headers.insert("WWW-Authenticate", Value::from("Challenge"));

            assert_same_headers(&headers, exception.get_headers());
        }

        #[test]
        fn testHeadersSetter() {
            for headers in header_data_provider() {
                let mut exception = HttpException::unauthorized("Challenge", HttpArgs::default());
                exception.set_headers(headers.clone());
                assert_same_headers(&headers, exception.get_headers());
            }
        }
    }
);
