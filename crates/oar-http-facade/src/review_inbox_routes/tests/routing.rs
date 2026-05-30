use hyper::http::Method;

use super::super::is_body_route;

#[test]
fn decision_body_route_predicate_only_matches_post_decisions() {
    assert!(is_body_route(&Method::POST, "/review-inbox/decisions"));
    assert!(!is_body_route(&Method::GET, "/review-inbox/decisions"));
    assert!(!is_body_route(&Method::POST, "/review-inbox/snapshot"));
    assert!(!is_body_route(&Method::POST, "/agent/stream"));
}
