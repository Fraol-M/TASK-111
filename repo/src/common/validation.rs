use validator::Validate;

use crate::common::errors::AppError;

/// Run `validator::Validate` on a DTO and map errors to AppError::UnprocessableEntity.
pub fn validate_dto<T: Validate>(dto: &T) -> Result<(), AppError> {
    dto.validate().map_err(|e| {
        let messages: Vec<String> = e
            .field_errors()
            .iter()
            .flat_map(|(field, errors)| {
                errors.iter().map(move |err| {
                    let msg = err
                        .message
                        .as_ref()
                        .map(|m| m.as_ref())
                        .unwrap_or("is invalid");
                    format!("{}: {}", field, msg)
                })
            })
            .collect();
        AppError::UnprocessableEntity(messages.join("; "))
    })
}
