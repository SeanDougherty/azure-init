// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

use std::process::Command;

use tracing::instrument;

use crate::{error::Error, imds::PublicKeys};

/// Configuration for the provisioned user.
#[derive(Clone)]
pub struct User {
    pub(crate) name: String,
    pub(crate) groups: Vec<String>,
    pub(crate) ssh_keys: Vec<PublicKeys>,
    pub(crate) password: Option<String>,
}

impl core::fmt::Debug for User {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // This is manually implemented to avoid printing the password if it's set
        f.debug_struct("User")
            .field("name", &self.name)
            .field("groups", &self.groups)
            .field("ssh_keys", &self.ssh_keys)
            .field("password", &self.password.is_some())
            .finish()
    }
}

impl User {
    /// Configure the user being provisioned on the host.
    pub fn new(
        name: impl Into<String>,
        ssh_keys: impl Into<Vec<PublicKeys>>,
    ) -> Self {
        Self {
            name: name.into(),
            groups: vec!["wheel".into()],
            ssh_keys: ssh_keys.into(),
            password: None,
        }
    }

    /// Set a password for the user; this is optional.
    pub fn with_password(mut self, password: impl Into<String>) -> Self {
        self.password = Some(password.into());
        self
    }

    /// A list of supplemental group names to add the user to.
    ///
    /// If any of the groups do not exist on the host, provisioning will fail.
    pub fn with_groups(mut self, groups: impl Into<Vec<String>>) -> Self {
        self.groups = groups.into();
        println!("{:#?}", self.groups);
        self
    }
}

#[derive(strum::EnumIter, Debug, Clone)]
#[non_exhaustive]
pub enum Provisioner {
    Useradd,
    #[cfg(test)]
    FakeUseradd,
}

impl Provisioner {
    pub(crate) fn create(&self, user: &User) -> Result<(), Error> {
        match self {
            Self::Useradd => useradd(user),
            #[cfg(test)]
            Self::FakeUseradd => Ok(()),
        }
    }
}

#[instrument(skip_all)]
fn useradd(user: &User) -> Result<(), Error> {
    let path_useradd = env!("PATH_USERADD");
    let home_path = format!("/home/{}", user.name);

    let status = Command::new(path_useradd)
                    .arg(&user.name)
                    .arg("--comment")
                    .arg(
                      "Provisioning agent created this user based on username provided in IMDS",
                    )
                    .arg("--groups")
                    .arg(user.groups.join(","))
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
