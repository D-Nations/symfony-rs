//! The application kernel on the ported Symfony kernel, served as nginx and
//! PHP-FPM would serve `public/index.php` (converter contract 0.3, sections
//! 6 and 7).
//!
//! Built once: the matcher over the routes (`FrameworkBundle`'s redirecting
//! matcher), the declared classes the resolver reads, and the
//! application-scoped container. Built per request, as PHP builds them:
//! the request-scoped container, the event dispatcher with the listeners
//! `FrameworkBundle` registers on this app's path (`RouterListener`,
//! `ResponseListener`) and the app's `ErrorSubscriber`, the
//! `ContainerControllerResolver` and `HttpKernel`.
//!
//! Not registered: `FrameworkBundle`'s other listeners, none of which
//! changes a recorded response, and `ErrorListener` with its error page,
//! which no recorded case reaches (an unhandled throwable is the runtime's
//! 500).
//!
//! Upstream: none

use std::{
    net::SocketAddr,
    sync::{Arc, LazyLock},
};

use axum::Router;
use php_value::Handle;
use symfony_event_dispatcher::{EventDispatcher, EventDispatcherInterface};
use symfony_http_foundation::{request_stack::RequestStack, sapi::Ini};
use symfony_http_kernel::{
    argument_resolver::ArgumentResolver,
    container_controller_resolver::{ContainerControllerResolver, ControllerContainer},
    controller_resolver::ClassRegistry,
    exception::Throwable,
    http_kernel::{HttpKernel, Options as KernelOptions},
    response_listener::ResponseListener,
    router_listener::{Matcher, Options as RouterOptions, RouterListener},
};
use symfony_routing::{
    redirectable_url_matcher::RedirectableUrlMatcher, request_context::RequestContext,
};
use symfony_runtime::{
    http_kernel_runner::RunnableKernel,
    server::{FrontController, ServerConfig},
};

use crate::{
    app::greet_controller,
    container::{Container, ServiceContainer},
    routes::routes,
    runtime::{
        NotPorted,
        framework_bundle::{RedirectableCompiledUrlMatcher, redirect_controller_class},
    },
};

/// What the app builds once.
struct App {
    matcher: Arc<RedirectableUrlMatcher<RedirectableCompiledUrlMatcher>>,
    classes: Arc<ClassRegistry>,
    container: Arc<Container>,
}

static APP: LazyLock<Result<App, String>> = LazyLock::new(|| {
    let routes = routes().map_err(|e| e.to_string())?;
    let matcher = Arc::new(RedirectableUrlMatcher::new(
        routes,
        RequestContext::default(),
        RedirectableCompiledUrlMatcher,
    ));
    let mut classes = ClassRegistry::new();
    classes
        .declare_class(greet_controller::class())
        .map_err(|e| e.to_string())?;
    classes
        .declare_class(redirect_controller_class())
        .map_err(|e| e.to_string())?;
    Ok(App {
        matcher,
        classes: Arc::new(classes),
        container: Arc::new(Container::new()),
    })
});

fn throwable(error: impl std::error::Error + Send + Sync + 'static) -> Throwable {
    Arc::new(error)
}

/// The kernel of one request, as `public/index.php` builds it.
pub fn kernel() -> Result<Arc<dyn RunnableKernel>, Throwable> {
    let app = APP
        .as_ref()
        .map_err(|_| throwable(NotPorted("the app failed to build")))?;
    let services = Arc::new(ServiceContainer::new(app.container.clone()));
    let request_stack = RequestStack::default();
    let dispatcher = Arc::new(EventDispatcher::new());
    // `router.request_context`: http_port 80, https_port 443.
    let listener = RouterListener::new(
        Matcher::Request(app.matcher.clone()),
        request_stack.clone(),
        RouterOptions {
            context: Some(Handle::new(RequestContext::default())),
            debug: false,
            ..RouterOptions::default()
        },
    )?;
    dispatcher
        .add_subscriber(Arc::new(listener))
        .map_err(throwable)?;
    // `kernel.charset`; `set_content_language_from_locale` is off.
    dispatcher
        .add_subscriber(Arc::new(ResponseListener::new("UTF-8", false)))
        .map_err(throwable)?;
    dispatcher
        .add_subscriber(Arc::new(app.container.error_subscriber()))
        .map_err(throwable)?;
    let resolver = ContainerControllerResolver::new(
        ControllerContainer::DependencyInjection(services),
        app.classes.clone(),
        None,
    );
    let kernel: Arc<dyn RunnableKernel> = HttpKernel::new(
        dispatcher,
        Arc::new(resolver),
        KernelOptions {
            request_stack: Some(request_stack),
            argument_resolver: Arc::new(ArgumentResolver::default()),
            // framework.handle_all_throwables: true
            handle_all_throwables: true,
        },
    );
    Ok(kernel)
}

/// The app served at `address` (`SERVER_ADDR`, `SERVER_PORT`), named
/// `server_name`.
pub fn router_at(address: SocketAddr, server_name: &str) -> Router {
    let server = ServerConfig {
        server_name: server_name.into(),
        server_addr: address,
        ..ServerConfig::default()
    };
    Arc::new(FrontController::new(
        server,
        Ini::default(),
        Arc::new(kernel),
    ))
    .router()
}

/// The app as the recording served it: `php -S 0.0.0.0:8000`.
pub fn router() -> Router {
    router_at(SocketAddr::from(([0, 0, 0, 0], 8000)), "0.0.0.0")
}
