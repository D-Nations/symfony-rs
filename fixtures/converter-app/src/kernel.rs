//! The application kernel: listeners registered at start-up with their
//! Symfony priorities, controller resolution from the router's match, and
//! invocation with the request scope (converter contract 0.1, sections 4
//! and 6).
//!
//! Upstream: none

use std::sync::Arc;

use axum::{
    body::Body,
    extract::Request,
    http::{HeaderValue, StatusCode, header},
    response::Response,
};
use php_value::PhpString;
use symfony_http_kernel::{Exception, Kernel};

use crate::{
    app::greet_controller,
    container::{Container, RequestScope},
    routes::{Route, routes},
    runtime::{
        framework_bundle::{UrlRedirect, url_redirect_action},
        http_foundation::{RequestInfo, Server},
        listeners::{Matched, ResponseListener, Routed, RouterListener},
    },
};

/// A resolved controller and its arguments.
pub enum Controller {
    Greet {
        info: RequestInfo,
        name: PhpString,
    },
    UrlRedirect {
        info: RequestInfo,
        redirect: UrlRedirect,
    },
}

pub struct App {
    container: Arc<Container>,
    kernel: Kernel<Controller>,
}

impl App {
    pub fn new(server: Server) -> Self {
        let container = Arc::new(Container::new());
        let mut kernel = Kernel::default();
        kernel.add_listener(
            32,
            RouterListener {
                routes: Arc::new(routes()),
                server,
            },
        );
        kernel.add_listener(0, container.error_subscriber());
        kernel.add_listener(0, ResponseListener { charset: "UTF-8" });
        Self { container, kernel }
    }

    /// One request, with a fresh request scope. An exception no listener
    /// handles would reach Symfony's ErrorListener and its error page,
    /// which the fixture does not port: it answers a bare 500 instead,
    /// and no recorded case reaches it.
    pub async fn handle(&self, request: Request) -> Response {
        let container = Arc::clone(&self.container);
        let scope = RequestScope::default();
        let result = self
            .kernel
            .handle(request, resolve, move |request, controller| {
                invoke(container, scope, request, controller)
            })
            .await;
        result.unwrap_or_else(unhandled)
    }
}

/// ControllerResolver: the controller the router chose.
fn resolve(request: &mut Request) -> Result<Controller, Exception> {
    let Some(Routed { info, matched }) = request.extensions_mut().remove::<Routed>() else {
        return Err(Exception::new(
            StatusCode::NOT_FOUND,
            format!(
                "Unable to find the controller for path \"{}\". The route is wrongly configured.",
                request.uri().path()
            ),
        ));
    };
    Ok(match matched {
        Matched::Route(Route::Greet { name }) => Controller::Greet {
            info,
            name: PhpString::from(name),
        },
        Matched::Redirect(redirect) => Controller::UrlRedirect { info, redirect },
    })
}

async fn invoke(
    container: Arc<Container>,
    scope: RequestScope,
    request: Request,
    controller: Controller,
) -> Result<Response, Exception> {
    match controller {
        Controller::Greet { name, .. } => {
            greet_controller::greet(&container, scope, request, name).await
        }
        Controller::UrlRedirect { info, redirect } => url_redirect_action(&info, &redirect),
    }
}

fn unhandled(exception: Exception) -> Response {
    let mut response = Response::new(Body::from("500 Internal Server Error\n"));
    *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/plain; charset=UTF-8"),
    );
    // Server-side only, as the kernel never writes `detail` to a response.
    eprintln!("unhandled exception: {}", exception.detail);
    response
}
