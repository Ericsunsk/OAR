use std::net::SocketAddr;

use crate::OarHttpFacadeConfig;

#[test]
fn config_defaults_to_localhost_and_accepts_docker_bind_override() {
    let default_config = OarHttpFacadeConfig::from_env_map(&|_| None).expect("default config");
    let docker_config = OarHttpFacadeConfig::from_env_map(&|key| {
        (key == "OAR_HTTP_BIND_ADDR").then(|| "0.0.0.0:8080".to_string())
    })
    .expect("docker config");

    assert_eq!(
        default_config.bind_addr,
        "127.0.0.1:8080".parse::<SocketAddr>().expect("addr")
    );
    assert_eq!(
        docker_config.bind_addr,
        "0.0.0.0:8080".parse::<SocketAddr>().expect("addr")
    );
}

#[test]
fn config_rejects_invalid_bind_override_without_echoing_in_display() {
    let error = OarHttpFacadeConfig::from_env_map(&|key| {
        (key == "OAR_HTTP_BIND_ADDR").then(|| "not an address".to_string())
    })
    .expect_err("invalid config");

    assert_eq!(
        error.to_string(),
        "oar_http_facade_config_invalid: invalid_bind_addr"
    );
    assert!(!error.to_string().contains("not an address"));
}
