use {
    semver::Version,
    serde::{
        Deserialize, Deserializer,
        de::{Error as DeError, Visitor},
    },
    std::{fmt, fs::read_to_string, path::PathBuf, str::FromStr},
};

#[derive(Debug, PartialEq, Eq)]
struct PackageVersion(String);

impl<'d> serde::Deserialize<'d> for PackageVersion {
    fn deserialize<D: Deserializer<'d>>(deserializer: D) -> Result<Self, D::Error> {
        struct PackageVersionVisitor;

        impl Visitor<'_> for PackageVersionVisitor {
            type Value = PackageVersion;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(formatter, "a semver string or a path to a package.json file")
            }

            fn visit_str<E: DeError>(self, value: &str) -> Result<PackageVersion, E> {
                let path = PathBuf::from(value);
                if path.exists() {
                    let json_str = read_to_string(&path)
                        .map_err(|e| DeError::custom(format!("failed to read version JSON file: {e}")))?;
                    let package_json: serde_json::Value = serde_json::from_str(&json_str)
                        .map_err(|e| DeError::custom(format!("failed to read version JSON file: {e}")))?;
                    if let Some(obj) = package_json.as_object() {
                        let version = obj
                            .get("version")
                            .ok_or_else(|| DeError::custom("JSON must contain a `version` field"))?
                            .as_str()
                            .ok_or_else(|| {
                                DeError::custom(format!("`{} > version` must be a string", path.display()))
                            })?;
                        Ok(PackageVersion(
                            Version::from_str(version)
                                .map_err(|_| DeError::custom("`tauri.conf.json > version` must be a semver string"))?
                                .to_string(),
                        ))
                    } else {
                        Err(DeError::custom("`tauri.conf.json > version` value is not a path to a JSON object"))
                    }
                } else {
                    Ok(PackageVersion(
                        Version::from_str(value)
                            .map_err(|_| DeError::custom("`tauri.conf.json > version` must be a semver string"))?
                            .to_string(),
                    ))
                }
            }
        }

        deserializer.deserialize_string(PackageVersionVisitor {})
    }
}

pub(crate) fn version_deserializer<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<PackageVersion>::deserialize(deserializer).map(|v| v.map(|v| v.0))
}
