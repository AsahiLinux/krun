use std::env;
use std::fs;
use std::net::{IpAddr, Ipv4Addr};
use std::str::FromStr;

use anyhow::{Context, Result};
use futures_util::TryStreamExt;
use rtnetlink::new_connection;
use rustix::system::sethostname;

pub async fn configure_network() -> Result<()> {
    {
        let hostname =
            fs::read_to_string("/etc/hostname").context("Failed to read `/etc/hostname`")?;
        let hostname = if let Some((hostname, _)) = hostname.split_once('\n') {
            hostname.to_owned()
        } else {
            hostname
        };
        sethostname(hostname.as_bytes()).context("Failed to set hostname")?;
    }

    let address = Ipv4Addr::from_str(
        &env::var("KRUN_NETWORK_ADDRESS").context("Missing KRUN_NETWORK_ADDRESS")?,
    )?;
    let mask = u32::from(Ipv4Addr::from_str(
        &env::var("KRUN_NETWORK_MASK").context("Missing KRUN_NETWORK_MASK")?,
    )?);
    let prefix = (!mask).leading_zeros() as u8;
    let router = env::var("KRUN_NETWORK_ROUTER").context("Missing KRUN_NETWORK_ROUTER")?;
    let router = Ipv4Addr::from_str(&router)?;

    let (connection, handle, _) = new_connection().unwrap();
    tokio::spawn(connection);
    let mut links = handle.link().get().match_name("eth0".to_string()).execute();
    if let Some(link) = links.try_next().await? {
        handle
            .address()
            .add(link.header.index, IpAddr::V4(address), prefix)
            .execute()
            .await?;
        handle.link().set(link.header.index).up().execute().await?
    }
    handle.route().add().v4().gateway(router).execute().await?;
    fs::write("/etc/resolv.conf", format!("nameserver {}", router))
        .expect("Failed to write resolv.conf");

    Ok(())
}
