// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.
pub mod hostname;
pub mod password;
pub(crate) mod ssh;
pub mod user;

use strum::IntoEnumIterator;
use tracing::instrument;

use crate::error::Error;

/// The interface for applying the desired configuration to the host.
///
/// Optional settings, like a password for the user account, can be provided
/// after constructing this object. By default, all known methods for
/// provisioning a particular setting are tried until one succeeds. Particular
/// methods can be selected via the `*_provisioners()` methods
/// ([`Provision::hostname_provisioners`], [`Provision::user_provisioners`],
/// etc).
///
/// To actually apply the configuration, use [`Provision::provision`].
#[derive(Clone)]
pub struct Provision {
    hostname: String,
    user: user::User,
    hostname_backends: Option<Vec<hostname::Provisioner>>,
    user_backends: Option<Vec<user::Provisioner>>,
    password_backends: Option<Vec<password::Provisioner>>,
}

impl Provision {
    pub fn new(hostname: impl Into<String>, user: user::User) -> Self {
        Self {
            hostname: hostname.into(),
            user,
            hostname_backends: None,
            user_backends: None,
            password_backends: None,
        }
    }

    /// Specify the ways to set the virtual machine's hostname.
    ///
    /// By default, all known methods will be attempted. Use this function to
    /// restrict which methods are attempted. These will be attempted in the
    /// order provided until one succeeds.
    pub fn hostname_provisioners(
        mut self,
        backends: impl Into<Vec<hostname::Provisioner>>,
    ) -> Self {
        self.hostname_backends = Some(backends.into());
        self
    }

    /// Specify the ways to create a user in the virtual machine
    ///
    /// By default, all known methods will be attempted. Use this function to
    /// restrict which methods are attempted. These will be attempted in the
    /// order provided until one succeeds.
    pub fn user_provisioners(
        mut self,
        backends: impl Into<Vec<user::Provisioner>>,
    ) -> Self {
        self.user_backends = Some(backends.into());
        self
    }

    /// Specify the ways to set a users password.
    ///
    /// By default, all known methods will be attempted. Use this function to
    /// restrict which methods are attempted. These will be attempted in the
    /// order provided until one succeeds. Only relevant if a password has been
    /// provided with [`Provision::password`].
    pub fn password_provisioners(
        mut self,
        backend: impl Into<Vec<password::Provisioner>>,
    ) -> Self {
        self.password_backends = Some(backend.into());
        self
    }

    /// Provision the host.
    #[instrument(skip_all)]
    pub fn provision(self) -> Result<(), Error> {
        self.user_backends
            .unwrap_or_else(|| user::Provisioner::iter().collect())
            .iter()
            .find_map(|backend| {
                backend
                    .create(&self.user)
                    .map_err(|e| {
                        tracing::info!(
                            error=?e,
                            backend=?backend,
                            resource="user",
                            "Provisioning did not succeed"
                        );
                        e
                    })
                    .ok()
            })
            .ok_or(Error::NoUserProvisioner)?;

        self.password_backends
            .unwrap_or_else(|| password::Provisioner::iter().collect())
            .iter()
            .find_map(|backend| {
                backend
                    .set(&self.user)
                    .map_err(|e| {
                        tracing::info!(
                            error=?e,
                            backend=?backend,
                            resource="password",
                            "Provisioning did not succeed"
                        );
                        e
                    })
                    .ok()
            })
            .ok_or(Error::NoPasswordProvisioner)?;

        if !self.user.ssh_keys.is_empty() {
            let user = nix::unistd::User::from_name(&self.user.name)?.ok_or(
                Error::UserMissing {
                    user: self.user.name,
                },
            )?;
            ssh::provision_ssh(&user, &self.user.ssh_keys)?;
        }

        self.hostname_backends
            .unwrap_or_else(|| hostname::Provisioner::iter().collect())
            .iter()
            .find_map(|backend| {
                backend
                    .set(&self.hostname)
                    .map_err(|e| {
                        tracing::info!(
                            error=?e,
                            backend=?backend,
                            resource="hostname",
                            "Provisioning did not succeed"
                        );
                        e
                    })
                    .ok()
            })
            .ok_or(Error::NoHostnameProvisioner)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use crate::User;

    use super::{hostname, password, user, Provision};

    #[test]
    fn test_successful_provision() {
        let _p = Provision::new(
            "my-hostname".to_string(),
            User::new("azureuser", vec![]),
        )
        .hostname_provisioners([hostname::Provisioner::FakeHostnamectl])
        .user_provisioners([user::Provisioner::FakeUseradd])
        .password_provisioners([password::Provisioner::FakePasswd])
        .provision()
        .unwrap();
    }
}
