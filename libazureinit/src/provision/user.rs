// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

use std::process::Command;

use tracing::instrument;

use crate::error::Error;

#[derive(strum::EnumIter, Debug, Clone)]
#[non_exhaustive]
pub enum Provisioner {
    Useradd,
    #[cfg(test)]
    FakeUseradd,
}

impl Provisioner {
    pub(crate) fn create(&self, name: impl AsRef<str>) -> Result<(), Error> {
        match self {
            Self::Useradd => useradd(name.as_ref()),
            #[cfg(test)]
            Self::FakeUseradd => Ok(()),
        }
    }
}

#[instrument(skip_all)]
fn useradd(name: &str) -> Result<(), Error> {
    let path_useradd = env!("PATH_USERADD");
    let home_path = format!("/home/{name}");

    let status = Command::new(path_useradd)
                    .arg(name)
                    .arg("--comment")
                    .arg(
                      "Provisioning agent created this user based on username provided in IMDS",
                    )
                    .arg("--groups")
                    .arg(env!("USERADD_GROUPS"))
                    .arg("-d")
                    .arg(home_path)
                    .arg("-m")
                    .status()?;
    if !status.success() {
        return Err(Error::SubprocessFailed {
            command: path_useradd.to_string(),
            status,
        });
    }

    Ok(())
}
