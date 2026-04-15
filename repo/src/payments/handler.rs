use actix_web::{web, HttpRequest, HttpResponse};
use uuid::Uuid;

use crate::common::{
    db::DbPool,
    errors::AppError,
    extractors::FinanceUser,
    pagination::{Page, PaginationParams},
    validation::validate_dto,
};
use crate::config::AppConfig;
use crate::payments::{dto::*, repository, service};

/// POST /payments/intents  (Finance/Admin — idempotency_key in request body)
pub async fn create_intent(
    _auth: FinanceUser,
    pool: web::Data<DbPool>,
    cfg: web::Data<AppConfig>,
    body: web::Json<CreateIntentRequest>,
) -> Result<HttpResponse, AppError> {
    validate_dto(&*body)?;

    let intent = service::create_intent(
        &pool,
        &cfg,
        body.booking_id,
        body.member_id,
        body.amount_cents,
        body.tax_cents,
        body.idempotency_key.clone(),
    )
    .await?;

    Ok(HttpResponse::Created().json(PaymentIntentResponse::from(intent)))
}

/// GET /payments/intents/{id}  (Finance/Admin)
pub async fn get_intent(
    _auth: FinanceUser,
    pool: web::Data<DbPool>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, AppError> {
    let intent_id = path.into_inner();

    let pool_c = pool.clone();
    let intent = actix_web::web::block(move || -> Result<_, AppError> {
        let mut conn = pool_c.get()?;
        repository::find_intent(&mut conn, intent_id)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;

    Ok(HttpResponse::Ok().json(PaymentIntentResponse::from(intent)))
}

/// POST /payments/intents/{id}/capture  (Finance/Admin — idempotency_key in request body)
pub async fn capture_payment(
    _auth: FinanceUser,
    pool: web::Data<DbPool>,
    path: web::Path<Uuid>,
    body: web::Json<CapturePaymentRequest>,
) -> Result<HttpResponse, AppError> {
    validate_dto(&*body)?;

    let intent_id = path.into_inner();
    let payment = service::capture_payment(
        &pool,
        intent_id,
        body.payment_method.clone(),
        body.idempotency_key.clone(),
        body.external_reference.clone(),
    )
    .await?;

    Ok(HttpResponse::Created().json(PaymentResponse::from(payment)))
}

/// POST /payments/{id}/refunds  (Finance/Admin — idempotency_key in request body)
pub async fn request_refund(
    req: HttpRequest,
    auth: FinanceUser,
    pool: web::Data<DbPool>,
    path: web::Path<Uuid>,
    body: web::Json<CreateRefundRequest>,
) -> Result<HttpResponse, AppError> {
    validate_dto(&*body)?;

    let payment_id = path.into_inner();
    let correlation_id = crate::common::correlation::get_correlation_id(&req);
    let refund = service::request_refund(
        &pool,
        payment_id,
        body.amount_cents,
        body.reason.clone(),
        body.idempotency_key.clone(),
        auth.0.sub,
        correlation_id,
    )
    .await?;

    Ok(HttpResponse::Created().json(RefundResponse::from(refund)))
}

/// PATCH /payments/refunds/{id}/approve  (Finance/Admin — emits audit)
pub async fn approve_refund(
    req: HttpRequest,
    auth: FinanceUser,
    pool: web::Data<DbPool>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, AppError> {
    let refund_id = path.into_inner();
    let correlation_id = crate::common::correlation::get_correlation_id(&req);
    let refund = service::approve_refund(&pool, refund_id, auth.0.sub, correlation_id).await?;
    Ok(HttpResponse::Ok().json(RefundResponse::from(refund)))
}

/// POST /payments/adjustments  (Finance/Admin — creates in pending state)
pub async fn create_adjustment(
    req: HttpRequest,
    auth: FinanceUser,
    pool: web::Data<DbPool>,
    body: web::Json<CreateAdjustmentRequest>,
) -> Result<HttpResponse, AppError> {
    validate_dto(&*body)?;

    let correlation_id = crate::common::correlation::get_correlation_id(&req);
    let adj = service::create_adjustment(
        &pool,
        body.payment_id,
        body.amount_cents,
        body.reason.clone(),
        auth.0.sub,
        correlation_id,
    )
    .await?;

    Ok(HttpResponse::Created().json(AdjustmentResponse::from(adj)))
}

/// PATCH /payments/adjustments/{id}/approve  (Finance/Admin)
pub async fn approve_adjustment(
    req: HttpRequest,
    auth: FinanceUser,
    pool: web::Data<DbPool>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, AppError> {
    let adj_id = path.into_inner();
    let correlation_id = crate::common::correlation::get_correlation_id(&req);
    let adj = service::approve_adjustment(&pool, adj_id, auth.0.sub, correlation_id).await?;
    Ok(HttpResponse::Ok().json(AdjustmentResponse::from(adj)))
}

/// GET /payments  (Finance/Admin)
pub async fn list_payments(
    _auth: FinanceUser,
    pool: web::Data<DbPool>,
    query: web::Query<PaginationParams>,
) -> Result<HttpResponse, AppError> {
    let limit = query.limit();
    let offset = query.offset();

    let pool_c = pool.clone();
    let (records, total) = actix_web::web::block(move || -> Result<_, AppError> {
        let mut conn = pool_c.get()?;
        repository::list_payments(&mut conn, limit, offset)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;

    let data: Vec<PaymentResponse> = records.into_iter().map(PaymentResponse::from).collect();
    Ok(HttpResponse::Ok().json(Page::new(data, total, &query)))
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/payments")
            .route("", web::get().to(list_payments))
            .route("/intents", web::post().to(create_intent))
            .route("/intents/{id}", web::get().to(get_intent))
            .route("/intents/{id}/capture", web::post().to(capture_payment))
            .route("/{id}/refunds", web::post().to(request_refund))
            .route("/refunds/{id}/approve", web::patch().to(approve_refund))
            .route("/adjustments", web::post().to(create_adjustment))
            .route("/adjustments/{id}/approve", web::patch().to(approve_adjustment)),
    );
}
