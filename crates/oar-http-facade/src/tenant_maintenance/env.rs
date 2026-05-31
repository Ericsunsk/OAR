use super::TenantMaintenanceSettingsError;
use crate::util::non_empty_env;

pub(super) fn enabled_env_flag(
    env: &impl Fn(&str) -> Option<String>,
    key: &str,
) -> Result<bool, TenantMaintenanceSettingsError> {
    let Some(value) = non_empty_env(env, key) else {
        return Ok(false);
    };
    match value.as_str() {
        "1" | "true" | "TRUE" | "yes" | "YES" => Ok(true),
        "0" | "false" | "FALSE" | "no" | "NO" => Ok(false),
        _ => Err(TenantMaintenanceSettingsError::InvalidConfig),
    }
}

pub(super) fn bounded_u64_env(
    env: &impl Fn(&str) -> Option<String>,
    key: &str,
    default_value: u64,
    min_value: u64,
    max_value: u64,
) -> Result<u64, TenantMaintenanceSettingsError> {
    let Some(value) = non_empty_env(env, key) else {
        return Ok(default_value);
    };
    let value = value
        .parse::<u64>()
        .map_err(|_| TenantMaintenanceSettingsError::InvalidConfig)?;
    if !(min_value..=max_value).contains(&value) {
        return Err(TenantMaintenanceSettingsError::InvalidConfig);
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enabled_env_flag_accepts_documented_literals() {
        for (value, expected) in [
            ("1", true),
            ("true", true),
            ("TRUE", true),
            ("yes", true),
            ("YES", true),
            ("0", false),
            ("false", false),
            ("FALSE", false),
            ("no", false),
            ("NO", false),
        ] {
            assert_eq!(
                enabled_env_flag(&single_env("FLAG", value), "FLAG").expect("valid flag"),
                expected
            );
        }
    }

    #[test]
    fn enabled_env_flag_rejects_ambiguous_literals() {
        for value in ["True", "Yes", "on", "enabled"] {
            assert_eq!(
                enabled_env_flag(&single_env("FLAG", value), "FLAG"),
                Err(TenantMaintenanceSettingsError::InvalidConfig)
            );
        }
    }

    #[test]
    fn bounded_u64_env_accepts_defaults_and_edges() {
        assert_eq!(
            bounded_u64_env(&|_| None, "LIMIT", 7, 1, 10).expect("default"),
            7
        );
        assert_eq!(
            bounded_u64_env(&single_env("LIMIT", "1"), "LIMIT", 7, 1, 10).expect("min"),
            1
        );
        assert_eq!(
            bounded_u64_env(&single_env("LIMIT", "10"), "LIMIT", 7, 1, 10).expect("max"),
            10
        );
        assert_eq!(
            bounded_u64_env(&single_env("LIMIT", " 5 "), "LIMIT", 7, 1, 10).expect("trimmed"),
            5
        );
    }

    #[test]
    fn bounded_u64_env_rejects_parse_and_range_failures() {
        for value in ["0", "11", "-1", "15s", "18446744073709551616"] {
            assert_eq!(
                bounded_u64_env(&single_env("LIMIT", value), "LIMIT", 7, 1, 10),
                Err(TenantMaintenanceSettingsError::InvalidConfig)
            );
        }
    }

    fn single_env<'a>(target: &'a str, value: &'a str) -> impl Fn(&str) -> Option<String> + 'a {
        move |key| (key == target).then(|| value.to_string())
    }
}
