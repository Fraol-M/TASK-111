/// Unit tests for idempotency hash computation.
#[cfg(test)]
mod idempotency_hash {
    use venue_booking::common::idempotency::hash_request;

    #[test]
    fn test_same_request_produces_same_hash() {
        let h1 = hash_request("POST", "/api/v1/bookings", b"{\"items\":[]}");
        let h2 = hash_request("POST", "/api/v1/bookings", b"{\"items\":[]}");
        assert_eq!(h1, h2, "Same request must produce identical hash");
    }

    #[test]
    fn test_different_method_produces_different_hash() {
        let h1 = hash_request("POST", "/api/v1/bookings", b"{}");
        let h2 = hash_request("GET", "/api/v1/bookings", b"{}");
        assert_ne!(h1, h2, "Different methods must produce different hashes");
    }

    #[test]
    fn test_different_path_produces_different_hash() {
        let h1 = hash_request("POST", "/api/v1/bookings", b"{}");
        let h2 = hash_request("POST", "/api/v1/payments", b"{}");
        assert_ne!(h1, h2, "Different paths must produce different hashes");
    }

    #[test]
    fn test_different_body_produces_different_hash() {
        let h1 = hash_request("POST", "/api/v1/bookings", b"{\"items\":[1]}");
        let h2 = hash_request("POST", "/api/v1/bookings", b"{\"items\":[2]}");
        assert_ne!(h1, h2, "Different bodies must produce different hashes");
    }

    #[test]
    fn test_hash_is_hex_string() {
        let h = hash_request("POST", "/test", b"body");
        assert!(!h.is_empty());
        // SHA-256 hex = 64 characters
        assert_eq!(h.len(), 64);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_empty_body_produces_valid_hash() {
        let h = hash_request("GET", "/health", b"");
        assert_eq!(h.len(), 64);
    }
}
