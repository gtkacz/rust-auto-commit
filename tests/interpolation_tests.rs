mod common;

use auto_commit_rs::config::AppConfig;
use auto_commit_rs::interpolation::interpolate;
use serial_test::serial;

use crate::common::EnvGuard;

#[test]
#[serial]
fn interpolate_replaces_known_variables_and_keeps_literals() {
    let cfg = AppConfig {
        provider: "openai".into(),
        model: "gpt-4o-mini".into(),
        api_key: "secret".into(),
        locale: "en".into(),
        ..Default::default()
    };

    let _env = EnvGuard::set(&[("CUSTOM_ENV", "custom")]);
    let result = interpolate(
        "provider=$ACR_PROVIDER model=$ACR_MODEL key=$ACR_API_KEY custom=$CUSTOM_ENV",
        &cfg,
    )
    .unwrap();

    assert_eq!(
        result,
        "provider=openai model=gpt-4o-mini key=secret custom=custom"
    );
}

#[test]
#[serial]
fn interpolate_rejects_missing_variables() {
    let cfg = AppConfig::default();
    let _env = EnvGuard::clear(&["DOES_NOT_EXIST"]);
    let error = interpolate("before:$DOES_NOT_EXIST:after", &cfg).unwrap_err();
    assert!(error.to_string().contains("DOES_NOT_EXIST"));
}

#[test]
#[serial]
fn interpolate_overrides_acr_variables_from_config_values() {
    let cfg = AppConfig {
        model: "model-from-config".into(),
        ..Default::default()
    };
    let _env = EnvGuard::set(&[("ACR_MODEL", "model-from-env")]);
    let result = interpolate("model=$ACR_MODEL", &cfg).unwrap();
    assert_eq!(result, "model=model-from-config");
    assert_eq!(
        std::env::var("ACR_MODEL").unwrap(),
        "model-from-env",
        "interpolation must not mutate process state"
    );
}
