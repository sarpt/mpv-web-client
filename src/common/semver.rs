use std::fmt::Display;

use serde::{Deserialize, Deserializer, Serialize};

#[derive(PartialEq, PartialOrd, Clone, Copy)]
pub struct Semver {
  major: usize,
  minor: usize,
  patch: usize,
}

const VERSION_SEMVER_SEPARATOR: &str = ".";
impl<'de> Deserialize<'de> for Semver {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    let version = String::deserialize(deserializer)?;
    Semver::from_string(&version).map_err(serde::de::Error::custom)
  }
}

impl Serialize for Semver {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: serde::Serializer,
  {
    String::serialize(&self.string_representation(), serializer)
  }
}

impl Semver {
  fn from_string(source: &String) -> Result<Self, String> {
    let mut split_version = source.split(VERSION_SEMVER_SEPARATOR).map(|chunk| {
      chunk
        .parse::<usize>()
        .map_err(|err| format!("could not parse source string of \"{source}\" as semver: {err}"))
    });
    let major: usize = split_version.next().unwrap_or(Ok(0))?;
    let minor: usize = split_version.next().unwrap_or(Ok(0))?;
    let patch: usize = split_version.next().unwrap_or(Ok(0))?;
    Ok(Semver {
      major,
      minor,
      patch,
    })
  }

  fn string_representation(&self) -> String {
    [self.major, self.minor, self.patch]
      .map(|chunk| chunk.to_string())
      .join(VERSION_SEMVER_SEPARATOR)
  }
}

impl TryFrom<&String> for Semver {
  type Error = String;

  fn try_from(value: &String) -> Result<Self, Self::Error> {
    Semver::from_string(value)
  }
}

impl TryFrom<String> for Semver {
  type Error = String;

  fn try_from(value: String) -> Result<Self, Self::Error> {
    Semver::from_string(&value)
  }
}

impl From<Semver> for String {
  fn from(val: Semver) -> Self {
    val.string_representation()
  }
}

impl Display for Semver {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.string_representation())
  }
}
