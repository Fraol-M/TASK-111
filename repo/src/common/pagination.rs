use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Clone)]
pub struct PaginationParams {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

impl PaginationParams {
    pub fn page(&self) -> i64 {
        self.page.unwrap_or(1).max(1)
    }

    pub fn per_page(&self) -> i64 {
        self.per_page.unwrap_or(20).clamp(1, 100)
    }

    pub fn offset(&self) -> i64 {
        (self.page() - 1) * self.per_page()
    }

    pub fn limit(&self) -> i64 {
        self.per_page()
    }
}

#[derive(Debug, Serialize)]
pub struct Page<T: Serialize> {
    pub data: Vec<T>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
    pub total_pages: i64,
}

impl<T: Serialize> Page<T> {
    pub fn new(data: Vec<T>, total: i64, params: &PaginationParams) -> Self {
        let per_page = params.per_page();
        let total_pages = if per_page > 0 {
            (total + per_page - 1) / per_page
        } else {
            0
        };
        Page {
            data,
            total,
            page: params.page(),
            per_page,
            total_pages,
        }
    }
}
