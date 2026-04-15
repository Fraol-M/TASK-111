use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    Error, HttpMessage,
};
use futures::future::{ok, LocalBoxFuture, Ready};
use uuid::Uuid;

pub struct CorrelationIdMiddleware;

impl<S, B> Transform<S, ServiceRequest> for CorrelationIdMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = CorrelationIdMiddlewareService<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(CorrelationIdMiddlewareService { service })
    }
}

pub struct CorrelationIdMiddlewareService<S> {
    service: S,
}

impl<S, B> Service<ServiceRequest> for CorrelationIdMiddlewareService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let correlation_id = req
            .headers()
            .get("X-Correlation-ID")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        // Store in request extensions for downstream access
        req.extensions_mut().insert(CorrelationId(correlation_id.clone()));

        let fut = self.service.call(req);
        let cid = correlation_id;

        Box::pin(async move {
            let mut res = fut.await?;
            res.headers_mut().insert(
                actix_web::http::header::HeaderName::from_static("x-correlation-id"),
                actix_web::http::header::HeaderValue::from_str(&cid)
                    .unwrap_or_else(|_| actix_web::http::header::HeaderValue::from_static("")),
            );
            Ok(res)
        })
    }
}

/// Request extension for downstream access to correlation ID.
#[derive(Clone)]
pub struct CorrelationId(pub String);

/// Extract correlation ID from request extensions.
pub fn get_correlation_id(req: &actix_web::HttpRequest) -> Option<String> {
    req.extensions().get::<CorrelationId>().map(|c| c.0.clone())
}
