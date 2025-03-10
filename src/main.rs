// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::Context;

use libazureinit::distro::{Distribution, Distributions};
use libazureinit::{
    error::Error as LibError,
    goalstate, imds, media,
    media::{Environment, Media},
    reqwest::{header, Client},
    user,
};

const VERSION: &str = env!("CARGO_PKG_VERSION");

// Mount the given device, get OVF environment data, return it.
fn mount_parse_ovf_env(dev: String) -> Result<Environment, anyhow::Error> {
    let mount_media =
        Media::new(PathBuf::from(dev), PathBuf::from(media::PATH_MOUNT_POINT));
    let mounted = mount_media
        .mount()
        .with_context(|| "Failed to mount media.")?;

    let ovf_body = mounted.read_ovf_env_to_string()?;
    let environment = media::parse_ovf_env(ovf_body.as_str())?;

    mounted
        .unmount()
        .with_context(|| "Failed to remove media.")?;

    Ok(environment)
}

fn get_username(imds_body: String) -> Result<String, anyhow::Error> {
    if imds::is_password_authentication_disabled(&imds_body)? {
        // password authentication is disabled
        Ok(imds::get_username(imds_body.clone())?)
    } else {
        // password authentication is enabled

        // list of CDROM devices that is available with possible filesystems.
        let ovf_devices = media::get_mount_device()?;
        let mut environment: Option<Environment> = None;

        // loop until it finds a correct device.
        for dev in ovf_devices {
            environment = match mount_parse_ovf_env(dev) {
                Ok(env) => Some(env),
                Err(_) => continue,
            }
        }

        Ok(environment
            .ok_or_else(|| {
                anyhow::anyhow!("Unable to get list of block devices")
            })?
            .provisioning_section
            .linux_prov_conf_set
            .username)
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    match provision().await {
        Ok(_) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{:?}", e);
            let config: u8 = exitcode::CONFIG
                .try_into()
                .expect("Error code must be less than 256");
            match e.root_cause().downcast_ref::<LibError>() {
                Some(LibError::UserMissing { user: _ }) => {
                    ExitCode::from(config)
                }
                Some(LibError::NonEmptyPassword) => ExitCode::from(config),
                Some(_) | None => ExitCode::FAILURE,
            }
        }
    }
}

async fn provision() -> Result<(), anyhow::Error> {
    let mut default_headers = header::HeaderMap::new();
    let user_agent = header::HeaderValue::from_str(
        format!("azure-init v{VERSION}").as_str(),
    )?;
    default_headers.insert(header::USER_AGENT, user_agent);
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .default_headers(default_headers)
        .build()?;
    let imds_body = imds::query_imds(&client).await?;
    let username = get_username(imds_body.clone())
        .with_context(|| "Failed to retrieve the admin username.")?;

    let mut file_path = "/home/".to_string();
    file_path.push_str(username.as_str());

    // always pass an empty password
    Distributions::from("ubuntu")
        .create_user(username.as_str(), "")
        .with_context(|| format!("Unabled to create user '{username}'"))?;

    user::create_ssh_directory(username.as_str(), &file_path)
        .await
        .with_context(|| "Failed to create ssh directory.")?;

    let keys = imds::get_ssh_keys(imds_body.clone())
        .with_context(|| "Failed to get ssh public keys.")?;

    file_path.push_str("/.ssh");

    user::set_ssh_keys(keys, username.to_string(), file_path.clone())
        .await
        .with_context(|| "Failed to write ssh public keys.")?;

    let hostname = imds::get_hostname(imds_body.clone())
        .with_context(|| "Failed to get the configured hostname")?;

    Distributions::from("ubuntu")
        .set_hostname(hostname.as_str())
        .with_context(|| "Failed to set hostname.")?;

    let vm_goalstate = goalstate::get_goalstate(&client)
        .await
        .with_context(|| "Failed to get desired goalstate.")?;
    goalstate::report_health(&client, vm_goalstate)
        .await
        .with_context(|| "Failed to report VM health.")?;

    Ok(())
}
